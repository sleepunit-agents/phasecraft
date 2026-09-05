use clap::{Parser, Subcommand};
use phasecraft::playback::ports::open_output;
use phasecraft::playback::transport::{PlayOptions, play};
use phasecraft::{
    music::{Composition, STEP_TICKS, resolve::resolve_step},
    playback::SilentOutput,
};
use std::{
    io::{self, Write},
    path::PathBuf,
    time::Duration,
};

#[derive(Parser)]
#[command(version = env!("PHASECRAFT_VERSION"), about = "Small deterministic musical systems, played live")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Report this executable's source commit and native platform.
    Version {
        #[arg(long)]
        json: bool,
    },
    /// Install the current rolling dev release in place (requires private repo access).
    Update {
        /// Compare commits without downloading or installing the executable.
        #[arg(long, conflicts_with = "force")]
        check: bool,
        /// Reinstall even when the installed commit matches.
        #[arg(long)]
        force: bool,
    },
    /// Create a project with playable 909 beats and separate musical libraries/config.
    New { directory: PathBuf },
    /// Validate a file, or every composition listed in a project (without MIDI).
    Validate {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Print available MIDI output destinations.
    Ports,
    /// Create Phasecraft ports using installed Windows MIDI Services tools.
    SetupMidi,
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
        #[arg(default_value = ".")]
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
        /// Send MIDI Clock and Start/Stop so the host follows this sequencer.
        #[arg(long)]
        send_clock: bool,
        /// Stop after this many 4/4 bars; omitted means continuous playback.
        #[arg(long)]
        bars: Option<u64>,
        #[arg(long)]
        watch: bool,
        /// Print planning provenance to stdout, ahead of actual playback.
        #[arg(long)]
        trace: bool,
        #[arg(long, value_parser = clap::value_parser!(u64).range(10..=1000))]
        lookahead_ms: Option<u64>,
    },
}
pub fn run() -> Result<(), String> {
    match Cli::parse().command {
        Command::Version { json } => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&phasecraft::update::version())
                        .map_err(|e| e.to_string())?
                );
            } else {
                println!("Phasecraft {}", env!("PHASECRAFT_VERSION"));
            }
            Ok(())
        }
        Command::Update { check, force } => phasecraft::update::run(check, force),
        Command::New { directory } => {
            phasecraft::authoring::project::create(&directory)?;
            println!(
                "Created {}. Edit config/midi.toml, then run phasecraft play from that folder.\nStart with phasecraft validate . or phasecraft play --dry-run --bars 4.",
                directory.display()
            );
            Ok(())
        }
        Command::Validate { path, json } => {
            let report = phasecraft::authoring::project::validate(&path);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
                );
            } else {
                for file in &report.files {
                    println!("Checked {file}");
                }
                for error in &report.errors {
                    eprintln!("{error}");
                }
                if report.valid {
                    println!("Valid ({} compositions)", report.files.len());
                }
            }
            if report.valid {
                Ok(())
            } else {
                Err("validation failed".into())
            }
        }
        Command::SetupMidi => {
            let port = phasecraft::playback::windows_setup::setup()?;
            println!("Send to {port}; in your host receive from Phasecraft Receive.");
            Ok(())
        }
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
                                let midi = phasecraft::music::resolve::to_midi(part, event);
                                format!(
                                    "note={} channel={} velocity={} gate={} ticks accent={:.2} onset_tick={}",
                                    part.output.note,
                                    part.output.channel,
                                    midi[0].bytes[2],
                                    event.duration_ticks,
                                    event.accent.amount,
                                    event.tick
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
            send_clock,
            bars,
            watch,
            trace,
            lookahead_ms,
        } => {
            let loaded = phasecraft::authoring::project::load(&file)?;
            let c = loaded.composition;
            let (port, virtual_port) = if port.is_some() || virtual_port {
                (port, virtual_port)
            } else {
                (loaded.midi.port, loaded.midi.virtual_port)
            };
            let lookahead_ms = lookahead_ms.unwrap_or(loaded.midi.lookahead_ms);
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
                send_clock: send_clock || loaded.midi.send_clock,
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
