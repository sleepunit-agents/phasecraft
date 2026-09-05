use clap::{Parser, Subcommand};
use phasecraft::{
    config::{Composition, MAX_PARTS, STEP_TICKS},
    engine::resolve_step,
    playback::{MidiOutput, MusicalClock, SilentOutput, dispatch_loop},
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

#[derive(Parser)]
#[command(version, about = "Small deterministic musical systems, played live")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Print available MIDI output destinations.
    Ports,
    /// Resolve steps as JSONL, including rests and their decision provenance.
    Inspect {
        file: PathBuf,
        #[arg(long, default_value_t = 64)]
        steps: u64,
        #[arg(long, default_value_t = 0)]
        start: u64,
        /// Readable per-Part decisions and resolved MIDI values.
        #[arg(long)]
        human: bool,
    },
    /// Print the fully expanded, validated composition with defaults.
    Expand { file: PathBuf },
    /// Loop all Parts until Ctrl-C; optionally reload at phrase boundaries.
    Play {
        file: PathBuf,
        /// Exact destination name, or its index from `ports`.
        #[arg(long, conflicts_with_all = ["virtual_port", "dry_run"])]
        port: Option<String>,
        /// Create a virtual source (macOS/Linux only).
        #[arg(long, conflicts_with = "dry_run")]
        virtual_port: bool,
        /// Exercise the realtime transport without a MIDI device.
        #[arg(long)]
        dry_run: bool,
        /// Stop after this many 4/4 bars; omitted means continuous playback.
        #[arg(long)]
        bars: Option<u64>,
        #[arg(long)]
        watch: bool,
        /// Print planning provenance to stdout, ahead of actual playback.
        #[arg(long)]
        trace: bool,
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u64).range(10..=1000))]
        lookahead_ms: u64,
    },
}
fn main() {
    if let Err(e) = run() {
        eprintln!("phasecraft: {e}");
        std::process::exit(1);
    }
}
fn open_output(
    port: Option<String>,
    virtual_port: bool,
) -> Result<midir::MidiOutputConnection, String> {
    let midi = midir::MidiOutput::new("Phasecraft").map_err(|e| e.to_string())?;
    if virtual_port {
        #[cfg(unix)]
        {
            use midir::os::unix::VirtualOutput;
            return midi.create_virtual("Phasecraft").map_err(|e| e.to_string());
        }
        #[cfg(not(unix))]
        {
            return Err("Virtual source creation is unavailable on this platform; use --port with an existing MIDI loopback destination".into());
        }
    }
    let name = port.ok_or("Choose --port NAME, --virtual-port, or --dry-run; use `phasecraft ports` to list destinations")?;
    let ports = midi.ports();
    let selected = if let Ok(index) = name.parse::<usize>() {
        ports.get(index)
    } else {
        ports
            .iter()
            .find(|p| midi.port_name(p).ok().as_deref() == Some(name.as_str()))
    };
    let selected = selected
        .ok_or_else(|| format!("MIDI output {name:?} not found; run `phasecraft ports`"))?;
    midi.connect(selected, "Phasecraft")
        .map_err(|e| e.to_string())
}
fn run() -> Result<(), String> {
    match Cli::parse().command {
        Command::Ports => {
            let midi = midir::MidiOutput::new("Phasecraft").map_err(|e| e.to_string())?;
            for (i, p) in midi.ports().iter().enumerate() {
                println!("{i}: {}", midi.port_name(p).map_err(|e| e.to_string())?);
            }
            Ok(())
        }
        Command::Expand { file } => {
            let c = Composition::read(&file)?;
            write!(
                io::stdout().lock(),
                "{}",
                toml::to_string_pretty(&c).map_err(|e| e.to_string())?
            )
            .map_err(|e| e.to_string())
        }
        Command::Inspect {
            file,
            steps,
            start,
            human,
        } => {
            let c = Composition::read(&file)?;
            let end = start
                .checked_add(steps)
                .filter(|end| *end <= u64::MAX / STEP_TICKS)
                .ok_or("step range overflows musical time")?;
            let mut out = io::BufWriter::new(io::stdout().lock());
            for step in start..end {
                for trace in resolve_step(&c, step).0 {
                    if human {
                        let part = c.parts.iter().find(|p| p.id == trace.part).unwrap();
                        let result = trace
                            .event
                            .as_ref()
                            .map(|event| {
                                let midi = phasecraft::engine::to_midi(part, event);
                                format!(
                                    "note={} channel={} velocity={} gate={} ticks accent={:.2}",
                                    part.output.note,
                                    part.output.channel,
                                    midi[0].bytes[2],
                                    event.duration_ticks,
                                    event.accent.amount
                                )
                            })
                            .unwrap_or_else(|| "rest".into());
                        writeln!(out, "{} {} | trigger pattern={} roll={:.4} p={:.2} fired={} | accent pattern={} roll={:.4} p={:.2} admitted={} | {}",
                            trace.position,trace.part,trace.trigger.rhythm.active(),trace.trigger.roll,trace.trigger.probability,trace.trigger.admitted,
                            trace.accent.rhythm.active(),trace.accent.roll,trace.accent.probability,trace.accent.admitted,result).map_err(|e|e.to_string())?;
                    } else {
                        serde_json::to_writer(&mut out, &trace).map_err(|e| e.to_string())?;
                        writeln!(out).map_err(|e| e.to_string())?;
                    }
                }
            }
            out.flush().map_err(|e| e.to_string())
        }
        Command::Play {
            file,
            port,
            virtual_port,
            dry_run,
            bars,
            watch,
            trace,
            lookahead_ms,
        } => {
            let c = Composition::read(&file)?;
            let steps = bars
                .map(|b| {
                    b.checked_mul(16)
                        .filter(|s| *s > 0 && *s <= u64::MAX / STEP_TICKS)
                        .ok_or("bars must be positive and fit in musical time")
                })
                .transpose()?;
            let options = PlayOptions {
                file,
                steps,
                watch,
                trace,
                lookahead: Duration::from_millis(lookahead_ms),
            };
            if dry_run {
                play(c, SilentOutput, options)
            } else {
                play(c, open_output(port, virtual_port)?, options)
            }
        }
    }
}
struct PlayOptions {
    file: PathBuf,
    steps: Option<u64>,
    watch: bool,
    trace: bool,
    lookahead: Duration,
}
fn play<S: MidiOutput + 'static>(
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
