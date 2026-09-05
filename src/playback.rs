use crate::{config::PPQN, engine::MidiEvent};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError},
    },
    time::{Duration, Instant},
};

pub trait MidiOutput: Send {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String>;
}
impl MidiOutput for midir::MidiOutputConnection {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.send(bytes).map_err(|e| e.to_string())
    }
}
pub struct SilentOutput;
impl MidiOutput for SilentOutput {
    fn send(&mut self, _: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct MusicalClock {
    pub tempo: f64,
}
impl MusicalClock {
    pub fn time_at_tick(self, tick: u64) -> Duration {
        Duration::from_secs_f64(tick as f64 * 60.0 / (self.tempo * PPQN as f64))
    }
}
#[derive(Default, Debug)]
pub struct DispatchStats {
    pub sent: u64,
    pub dropped_late_notes: u64,
    pub max_lateness: Duration,
}
/// Tracks only our active notes; cleanup does not silence other users of a port.
pub struct EventDispatcher<S: MidiOutput> {
    pub sink: S,
    active: BTreeSet<(u8, u8)>,
    pub stats: DispatchStats,
}
impl<S: MidiOutput> EventDispatcher<S> {
    pub fn new(sink: S) -> Self {
        Self {
            sink,
            active: BTreeSet::new(),
            stats: DispatchStats::default(),
        }
    }
    pub fn dispatch(
        &mut self,
        event: &MidiEvent,
        lateness: Duration,
        late_limit: Duration,
    ) -> Result<(), String> {
        let [status, note, velocity] = event.bytes;
        let key = (status & 15, note);
        let is_on = status & 0xf0 == 0x90 && velocity > 0;
        self.stats.max_lateness = self.stats.max_lateness.max(lateness);
        if is_on && lateness > late_limit {
            self.stats.dropped_late_notes += 1;
            return Ok(());
        }
        if is_on {
            // Register before sending: even a partially failed send merits cleanup.
            self.active.insert(key);
            self.sink.send(&event.bytes)?;
        } else {
            if !self.active.contains(&key) {
                return Ok(());
            }
            self.sink.send(&event.bytes)?;
            self.active.remove(&key);
        }
        self.stats.sent += 1;
        Ok(())
    }
    pub fn cleanup(&mut self) -> Result<(), String> {
        let mut error = None;
        for &(channel, note) in &self.active {
            if let Err(e) = self.sink.send(&[0x80 | channel, note, 0]) {
                error = Some(e);
            }
        }
        self.active.clear();
        error.map_or(Ok(()), Err)
    }
}
impl<S: MidiOutput> Drop for EventDispatcher<S> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

/// Only this thread touches the output. Deadlines are absolute, so oversleep
/// never shifts future notes. Generation, parsing and logging run elsewhere.
pub fn dispatch_loop<S: MidiOutput>(
    sink: S,
    rx: Receiver<MidiEvent>,
    running: Arc<AtomicBool>,
    origin: Instant,
    clock: MusicalClock,
    late_limit: Duration,
) -> Result<DispatchStats, String> {
    let mut dispatcher = EventDispatcher::new(sink);
    let result = (|| {
        while running.load(Ordering::Relaxed) {
            let event = match rx.recv_timeout(Duration::from_millis(10)) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            };
            let deadline = origin + clock.time_at_tick(event.tick);
            loop {
                if !running.load(Ordering::Relaxed) {
                    return Ok(());
                }
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                std::thread::sleep((deadline - now).min(Duration::from_millis(2)));
            }
            dispatcher.dispatch(
                &event,
                Instant::now().saturating_duration_since(deadline),
                late_limit,
            )?;
        }
        Ok(())
    })();
    running.store(false, Ordering::Relaxed);
    let cleanup = dispatcher.cleanup();
    result.and(cleanup)?;
    Ok(std::mem::take(&mut dispatcher.stats))
}
