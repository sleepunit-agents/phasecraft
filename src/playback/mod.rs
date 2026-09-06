pub mod ports;
pub mod sync;
pub mod transport;
use crate::music::{PPQN, resolve::MidiEvent};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, TryRecvError},
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
    pub controls_sent: u64,
    pub clock_pulses: u64,
    pub dropped_late_notes: u64,
    pub max_lateness: Duration,
}
/// Tracks only our active notes; cleanup does not silence other users of a port.
pub struct EventDispatcher<S: MidiOutput> {
    pub sink: S,
    active: BTreeSet<(u8, u8)>,
    controls: BTreeMap<(u8, u8), u8>,
    last_controls: BTreeMap<(u8, u8), u8>,
    stop_controls: BTreeMap<(u8, u8), u8>,
    pub stats: DispatchStats,
}
impl<S: MidiOutput> EventDispatcher<S> {
    pub fn new(sink: S) -> Self {
        Self {
            sink,
            active: BTreeSet::new(),
            controls: BTreeMap::new(),
            last_controls: BTreeMap::new(),
            stop_controls: BTreeMap::new(),
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
        if status & 0xf0 == 0xb0 {
            // Ignore obsolete timeline points, but release an outstanding emphasis.
            if event.parameter
                && lateness > late_limit
                && (event.reset_value.is_some() || !self.controls.contains_key(&key))
            {
                return Ok(());
            }
            if let Some(reset) = event.reset_value {
                if lateness > late_limit {
                    return Ok(());
                }
                self.controls.insert(key, reset);
            }
            if let Some(value) = event.stop_value {
                self.stop_controls.insert(key, value);
            }
            if !event.parameter || self.last_controls.get(&key) != Some(&velocity) {
                self.sink.send(&event.bytes)?;
                self.last_controls.insert(key, velocity);
                self.stats.controls_sent += 1;
            }
            if event.reset_value.is_none() {
                self.controls.remove(&key);
            }
            return Ok(());
        }
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
        // Transport defaults take precedence over a temporary accent base.
        let mut resets = std::mem::take(&mut self.controls);
        resets.extend(std::mem::take(&mut self.stop_controls));
        for (&(channel, cc), &reset) in &resets {
            if let Err(e) = self.sink.send(&[0xb0 | channel, cc, reset]) {
                error = Some(e);
            } else {
                self.last_controls.insert((channel, cc), reset);
            }
        }
        self.controls.clear();
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
    dispatch_loop_with_sync(
        sink,
        rx,
        running,
        origin,
        clock,
        late_limit,
        sync::SyncOptions::default(),
    )
}

pub fn dispatch_loop_with_sync<S: MidiOutput>(
    sink: S,
    rx: Receiver<MidiEvent>,
    running: Arc<AtomicBool>,
    origin: Instant,
    clock: MusicalClock,
    late_limit: Duration,
    options: sync::SyncOptions,
) -> Result<DispatchStats, String> {
    let mut dispatcher = EventDispatcher::new(sink);
    let mut sync = sync::ClockOutput::new(origin, clock, options);
    let result = (|| {
        let mut pending = None;
        while running.load(Ordering::Relaxed) {
            if pending.is_none() {
                match rx.try_recv() {
                    Ok(event) => pending = Some(event),
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => break,
                }
            }
            let note_deadline = pending
                .as_ref()
                .map(|e| origin + clock.time_at_tick(e.tick));
            let pulse_deadline = sync.deadline();
            let now = Instant::now();
            let deadline = note_deadline
                .into_iter()
                .chain(pulse_deadline)
                .min()
                .unwrap_or(now + Duration::from_millis(2));
            if now < deadline {
                std::thread::sleep((deadline - now).min(Duration::from_millis(2)));
                continue;
            }
            // At a shared deadline, Start/Clock precede the first note.
            if pulse_deadline.is_some_and(|d| d == deadline) {
                sync.send(&mut dispatcher.sink, now)?;
            } else if let Some(event) = pending.take() {
                dispatcher.dispatch(&event, now.saturating_duration_since(deadline), late_limit)?;
            }
        }
        Ok(())
    })();
    running.store(false, Ordering::Relaxed);
    let cleanup = dispatcher.cleanup();
    let stop = sync.stop(&mut dispatcher.sink);
    dispatcher.stats.clock_pulses = sync.pulses();
    result.and(cleanup).and(stop)?;
    Ok(std::mem::take(&mut dispatcher.stats))
}

impl MidiOutput for Box<dyn MidiOutput> {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        (**self).send(bytes)
    }
}

pub mod controller;
