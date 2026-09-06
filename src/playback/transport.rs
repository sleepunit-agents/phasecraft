use crate::{
    music::{Composition, MAX_PARTS, STEP_TICKS, resolve::resolve_step},
    playback::{MidiOutput, MusicalClock, dispatch_loop_with_sync, sync::SyncOptions},
};
use std::{
    io::{self, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
pub struct PlayOptions {
    pub file: PathBuf,
    pub steps: Option<u64>,
    pub watch: bool,
    pub trace: bool,
    pub send_clock: bool,
    pub lookahead: Duration,
}
pub fn play<S: MidiOutput + 'static>(
    c: Composition,
    sink: S,
    options: PlayOptions,
) -> Result<(), String> {
    let running = Arc::new(AtomicBool::new(true));
    let stop = running.clone();
    ctrlc::set_handler(move || stop.store(false, Ordering::Relaxed)).map_err(|e| e.to_string())?;
    run_controlled(c, sink, options, running, None)
}

/// Planned snapshots carry their audible deadlines; consumers must not display them early.
/// Dropping telemetry under load never blocks MIDI planning or dispatch.
pub struct PlaybackFrame {
    pub deadline: Instant,
    pub step: u64,
    pub composition: Arc<Composition>,
    pub traces: Vec<crate::music::resolve::StepTrace>,
    pub reload_error: Option<String>,
}

pub fn run_controlled<S: MidiOutput + 'static>(
    c: Composition,
    sink: S,
    options: PlayOptions,
    running: Arc<AtomicBool>,
    feedback: Option<mpsc::SyncSender<PlaybackFrame>>,
) -> Result<(), String> {
    run_with_controls(c, sink, options, running, feedback, None)
}

pub fn run_with_controls<S: MidiOutput + 'static>(
    mut c: Composition,
    sink: S,
    options: PlayOptions,
    running: Arc<AtomicBool>,
    feedback: Option<mpsc::SyncSender<PlaybackFrame>>,
    live: Option<crate::control::Shared>,
) -> Result<(), String> {
    let end_steps = match (options.steps, c.end_step()) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    let clock = MusicalClock { tempo: c.tempo };
    let origin = Instant::now() + options.lookahead;
    let ahead_steps = (options.lookahead.as_secs_f64()
        / clock.time_at_tick(STEP_TICKS).as_secs_f64())
    .ceil() as usize;
    let (tx, rx) = mpsc::sync_channel(
        (2 + (crate::music::parameter::MAX_SAMPLES_PER_STEP + 1)
            * crate::music::accent::MAX_CONTROLS)
            * MAX_PARTS
            * (ahead_steps + 2),
    );
    let dispatch_running = running.clone();
    // Late threshold is shorter than one sixteenth even at the maximum BPM.
    let worker = std::thread::spawn(move || {
        dispatch_loop_with_sync(
            sink,
            rx,
            dispatch_running,
            origin,
            clock,
            Duration::from_millis(20),
            SyncOptions {
                enabled: options.send_clock,
                end_tick: end_steps.map(|s| s * STEP_TICKS),
            },
        )
    });
    eprintln!(
        "Playing {} Parts at {} BPM; seed {}; Ctrl-C stops.{}",
        c.parts.len(),
        c.tempo,
        c.seed,
        if options.watch {
            " Watching at phrase boundaries (planned ahead)."
        } else {
            ""
        }
    );
    let mut last_source = serde_json::to_string(&c).map_err(|e| e.to_string())?;
    let mut last_error = String::new();
    let mut visual_composition = Arc::new(c.clone());
    let mut skipped_steps = 0u64;
    let mut live_revision = None;
    let planned = (|| {
        let mut step = 0;
        let mut last_scheduled: Option<(u64, Arc<Composition>)> = None;
        while running.load(Ordering::Relaxed) && end_steps.is_none_or(|end| step < end) {
            let deadline = origin + clock.time_at_tick(step * STEP_TICKS);
            if deadline > Instant::now() + options.lookahead {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            if options.watch && step > 0 && step % c.phrase_steps() == 0 {
                let reload = Composition::read(&options.file).and_then(|next| {
                    if next.tempo != c.tempo || next.phrase_bars != c.phrase_bars {
                        return Err("restart playback to change tempo or phrase_bars".into());
                    }
                    if !c.same_arrangement_layout(&next) {
                        return Err("restart playback to change arrangement layout, phrase lengths or routing".into());
                    }
                    Ok(next)
                });
                match reload {
                    Ok(next) => {
                        last_error.clear();
                        let source = serde_json::to_string(&next).map_err(|e| e.to_string())?;
                        if source != last_source {
                            last_source = source;
                            c = next;
                            if let Some(live) = &live {
                                let mut live = live.lock().map_err(|e| e.to_string())?;
                                live.rebase(c.clone());
                                live.reset();
                            }
                            visual_composition = Arc::new(c.clone());
                            eprintln!(
                                "Applied configuration at bar {} (seed {}).",
                                step / 16 + 1,
                                c.seed
                            );
                        }
                    }
                    Err(error) if error != last_error => {
                        eprintln!("Reload rejected; continuing previous configuration: {error}");
                        last_error = error;
                    }
                    Err(_) => {}
                }
            }
            if step % 16 == 0
                && let Some(live) = &live
            {
                let live = live.lock().map_err(|e| e.to_string())?;
                if live_revision != Some(live.revision) {
                    if let Some(next) = live.composition() {
                        c = next;
                        visual_composition = Arc::new(c.clone());
                    }
                    live_revision = Some(live.revision);
                }
            }
            // After a stalled producer, jump over obsolete positions. Stateless
            // phase/decision addressing retains the original transport history.
            if Instant::now().saturating_duration_since(deadline) > Duration::from_millis(20) {
                skipped_steps += 1;
                step += 1;
                continue;
            }
            let (traces, mut events) = resolve_step(&c, step);
            // Handle a producer stall that jumped over the exact section boundary.
            // Use the last actually scheduled snapshot, not a guessed previous section.
            if let (Some(a), Some((previous_step, previous))) = (&c.arrangement, &last_scheduled) {
                let current = a.locate(step).map(|s| (s.position.index, s.position.cycle));
                let old = previous
                    .arrangement
                    .as_ref()
                    .and_then(|a| a.locate(*previous_step))
                    .map(|s| (s.position.index, s.position.cycle));
                if current != old {
                    events.retain(|e| !e.boundary_reset);
                    events.extend(crate::music::arrangement::resets(
                        previous.at_step(*previous_step),
                        step * STEP_TICKS,
                    ));
                    events.sort_by_key(crate::music::resolve::midi_order);
                }
            }
            if c.arrangement.is_none()
                && let Some((_, previous)) = &last_scheduled
                && !Arc::ptr_eq(previous, &visual_composition)
            {
                events.extend(crate::control::removed_control_resets(
                    previous,
                    &c,
                    step * STEP_TICKS,
                ));
                events.sort_by_key(crate::music::resolve::midi_order);
            }
            let changed = last_scheduled
                .as_ref()
                .is_none_or(|(_, previous)| !Arc::ptr_eq(previous, &visual_composition));
            last_scheduled = Some((step, visual_composition.clone()));
            for midi in events {
                tx.try_send(midi)
                    .map_err(|e| format!("MIDI dispatch queue: {e}"))?;
            }
            if changed && let Some(live) = &live {
                live.lock()
                    .map_err(|e| e.to_string())?
                    .schedule(c.clone(), deadline);
            }
            if options.trace {
                for trace in &traces {
                    writeln!(
                        io::stdout().lock(),
                        "{}",
                        serde_json::to_string(&trace).map_err(|e| e.to_string())?
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
            if let Some(feedback) = &feedback {
                let _ = feedback.try_send(PlaybackFrame {
                    deadline,
                    step,
                    composition: visual_composition.clone(),
                    traces,
                    reload_error: (!last_error.is_empty()).then(|| last_error.clone()),
                });
            }
            step += 1;
        }
        // A finite run owns its full bar duration, including trailing rests.
        if let Some(end) = end_steps {
            let finish = origin + clock.time_at_tick(end * STEP_TICKS);
            while running.load(Ordering::Relaxed) && Instant::now() < finish {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        Ok(())
    })();
    if planned.is_err() {
        running.store(false, Ordering::Relaxed);
    }
    drop(tx);
    let dispatched = worker
        .join()
        .map_err(|_| "MIDI dispatch thread panicked".to_string())?;
    let stats = dispatched?;
    eprintln!(
        "Stopped: {} note messages, {} control messages, {} clock pulses, {} late notes dropped, maximum dispatch lateness {:.3} ms.",
        stats.sent,
        stats.controls_sent,
        stats.clock_pulses,
        stats.dropped_late_notes,
        stats.max_lateness.as_secs_f64() * 1000.0
    );
    if skipped_steps > 0 {
        eprintln!("Producer missed {skipped_steps} grid positions after stalls.");
    }
    planned
}
