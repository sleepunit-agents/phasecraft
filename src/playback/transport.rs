use crate::{
    music::{Composition, MAX_PARTS, STEP_TICKS, resolve::resolve_step},
    playback::{MidiOutput, MusicalClock, dispatch_loop},
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
    pub lookahead: Duration,
}
pub fn play<S: MidiOutput + 'static>(
    mut c: Composition,
    sink: S,
    options: PlayOptions,
) -> Result<(), String> {
    let running = Arc::new(AtomicBool::new(true));
    let stop = running.clone();
    ctrlc::set_handler(move || stop.store(false, Ordering::Relaxed)).map_err(|e| e.to_string())?;
    let clock = MusicalClock { tempo: c.tempo };
    let origin = Instant::now() + options.lookahead;
    let ahead_steps = (options.lookahead.as_secs_f64()
        / clock.time_at_tick(STEP_TICKS).as_secs_f64())
    .ceil() as usize;
    let (tx, rx) = mpsc::sync_channel(2 * MAX_PARTS * (ahead_steps + 2));
    let dispatch_running = running.clone();
    // Late threshold is shorter than one sixteenth even at the maximum BPM.
    let worker = std::thread::spawn(move || {
        dispatch_loop(
            sink,
            rx,
            dispatch_running,
            origin,
            clock,
            Duration::from_millis(20),
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
    let mut skipped_steps = 0u64;
    let planned = (|| {
        let mut step = 0;
        while running.load(Ordering::Relaxed) && options.steps.is_none_or(|end| step < end) {
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
                    Ok(next)
                });
                match reload {
                    Ok(next) => {
                        last_error.clear();
                        let source = serde_json::to_string(&next).map_err(|e| e.to_string())?;
                        if source != last_source {
                            last_source = source;
                            c = next;
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
            // After a stalled producer, jump over obsolete positions. Stateless
            // phase/decision addressing retains the original transport history.
            if Instant::now().saturating_duration_since(deadline) > Duration::from_millis(20) {
                skipped_steps += 1;
                step += 1;
                continue;
            }
            let (traces, events) = resolve_step(&c, step);
            for midi in events {
                tx.try_send(midi)
                    .map_err(|e| format!("MIDI dispatch queue: {e}"))?;
            }
            if options.trace {
                for trace in traces {
                    writeln!(
                        io::stdout().lock(),
                        "{}",
                        serde_json::to_string(&trace).map_err(|e| e.to_string())?
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
            step += 1;
        }
        // A finite run owns its full bar duration, including trailing rests.
        if let Some(end) = options.steps {
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
        "Stopped: {} messages sent, {} late notes dropped, maximum dispatch lateness {:.3} ms.",
        stats.sent,
        stats.dropped_late_notes,
        stats.max_lateness.as_secs_f64() * 1000.0
    );
    if skipped_steps > 0 {
        eprintln!("Producer missed {skipped_steps} grid positions after stalls.");
    }
    planned
}
