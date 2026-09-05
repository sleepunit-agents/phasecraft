//! MIDI clock is a transport lane, independent of notes, Parts, and their probabilities.
use super::{MidiOutput, MusicalClock};
use crate::music::PPQN;
use std::time::Instant;

pub const CLOCK_TICKS: u64 = PPQN / 24;
#[derive(Clone, Copy, Default)]
pub struct SyncOptions {
    pub enabled: bool,
    pub end_tick: Option<u64>,
}
pub struct ClockOutput {
    origin: Instant,
    clock: MusicalClock,
    options: SyncOptions,
    pulse: u64,
    started: bool,
}
impl ClockOutput {
    pub fn new(origin: Instant, clock: MusicalClock, options: SyncOptions) -> Self {
        Self {
            origin,
            clock,
            options,
            pulse: 0,
            started: false,
        }
    }
    pub fn deadline(&self) -> Option<Instant> {
        let tick = self.pulse * CLOCK_TICKS;
        (self.options.enabled && self.options.end_tick.is_none_or(|end| tick < end))
            .then(|| self.origin + self.clock.time_at_tick(tick))
    }
    pub fn send(&mut self, sink: &mut impl MidiOutput, now: Instant) -> Result<(), String> {
        let Some(deadline) = self.deadline() else {
            return Ok(());
        };
        if now < deadline {
            return Ok(());
        }
        // Never send a burst of obsolete clock pulses: it would mislead the receiving clock.
        if now.saturating_duration_since(deadline) >= self.clock.time_at_tick(CLOCK_TICKS) {
            return Err("MIDI clock missed a pulse deadline; playback stopped to avoid sending an incorrect tempo. Restart playback.".into());
        }
        if !self.started {
            self.started = true; // Attempt Stop even if Start partially fails.
            sink.send(&[0xfa])?;
        }
        sink.send(&[0xf8])?;
        self.pulse += 1;
        Ok(())
    }
    pub fn stop(&mut self, sink: &mut impl MidiOutput) -> Result<(), String> {
        if std::mem::take(&mut self.started) {
            sink.send(&[0xfc])?;
        }
        Ok(())
    }
    pub fn pulses(&self) -> u64 {
        self.pulse
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[derive(Default)]
    struct Capture(Vec<Vec<u8>>);
    impl MidiOutput for Capture {
        fn send(&mut self, b: &[u8]) -> Result<(), String> {
            self.0.push(b.to_vec());
            Ok(())
        }
    }
    #[test]
    fn exact_clock_count_at_techno_and_dnb_tempos_including_silent_bars() {
        for tempo in [132., 172.] {
            let origin = Instant::now();
            let clock = MusicalClock { tempo };
            let mut lane = ClockOutput::new(
                origin,
                clock,
                SyncOptions {
                    enabled: true,
                    end_tick: Some(PPQN * 4 * 4),
                },
            );
            let mut sink = Capture::default();
            for pulse in 0..384 {
                let deadline = origin + clock.time_at_tick(pulse * CLOCK_TICKS);
                assert_eq!(lane.deadline(), Some(deadline));
                lane.send(&mut sink, deadline).unwrap();
            }
            assert!(lane.deadline().is_none());
            lane.stop(&mut sink).unwrap();
            lane.stop(&mut sink).unwrap();
            assert_eq!(sink.0.first().unwrap(), &[0xfa]);
            assert_eq!(sink.0.last().unwrap(), &[0xfc]);
            assert_eq!(
                sink.0.iter().filter(|b| b.as_slice() == [0xf8]).count(),
                384
            );
            assert_eq!(sink.0.len(), 386);
        }
    }
    #[test]
    fn disabled_early_cancel_and_stalls_do_not_emit_clock_bursts() {
        let origin = Instant::now();
        let clock = MusicalClock { tempo: 172. };
        let mut sink = Capture::default();
        let mut disabled = ClockOutput::new(origin, clock, SyncOptions::default());
        disabled.send(&mut sink, origin).unwrap();
        disabled.stop(&mut sink).unwrap();
        assert!(sink.0.is_empty());
        let mut lane = ClockOutput::new(
            origin,
            clock,
            SyncOptions {
                enabled: true,
                end_tick: None,
            },
        );
        lane.send(&mut sink, origin - Duration::from_millis(1))
            .unwrap();
        lane.stop(&mut sink).unwrap();
        assert!(sink.0.is_empty());
        lane.send(&mut sink, origin).unwrap();
        assert!(
            lane.send(&mut sink, origin + Duration::from_secs(1))
                .is_err()
        );
        lane.stop(&mut sink).unwrap();
        assert_eq!(sink.0, vec![vec![0xfa], vec![0xf8], vec![0xfc]]);
    }
}
