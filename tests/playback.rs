use phasecraft::{engine::MidiEvent, playback::*};
use std::{
    sync::{Arc, Mutex, atomic::AtomicBool, mpsc},
    time::{Duration, Instant},
};
#[derive(Clone, Default)]
struct Recording {
    messages: Arc<Mutex<Vec<Vec<u8>>>>,
}
impl MidiOutput for Recording {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.messages.lock().unwrap().push(bytes.to_vec());
        Ok(())
    }
}
fn on(tick: u64) -> MidiEvent {
    MidiEvent {
        stop_value: None,
        reset_value: None,
        parameter: false,
        tick,
        bytes: [0x99, 42, 100],
    }
}
fn off(tick: u64) -> MidiEvent {
    MidiEvent {
        stop_value: None,
        reset_value: None,
        parameter: false,
        tick,
        bytes: [0x89, 42, 0],
    }
}
#[test]
fn musical_clock_converts_from_an_absolute_origin() {
    let clock = MusicalClock { tempo: 120.0 };
    assert_eq!(clock.time_at_tick(960), Duration::from_millis(500));
    assert_eq!(clock.time_at_tick(240 * 560), Duration::from_secs(70));
}
#[test]
fn late_notes_are_dropped_but_active_note_offs_are_delivered() {
    let mut d = EventDispatcher::new(Recording::default());
    let limit = Duration::from_millis(20);
    d.dispatch(&on(0), Duration::ZERO, limit).unwrap();
    d.dispatch(&off(120), Duration::from_secs(1), limit)
        .unwrap();
    d.dispatch(&on(240), Duration::from_secs(1), limit).unwrap();
    d.dispatch(&off(360), Duration::ZERO, limit).unwrap();
    assert_eq!(
        *d.sink.messages.lock().unwrap(),
        vec![vec![0x99, 42, 100], vec![0x89, 42, 0]]
    );
    assert_eq!(d.stats.dropped_late_notes, 1);
}
#[test]
fn cleanup_sends_only_owned_note_offs_and_is_idempotent() {
    let mut d = EventDispatcher::new(Recording::default());
    d.dispatch(&on(0), Duration::ZERO, Duration::ZERO).unwrap();
    d.cleanup().unwrap();
    d.cleanup().unwrap();
    assert_eq!(
        *d.sink.messages.lock().unwrap(),
        vec![vec![0x99, 42, 100], vec![0x89, 42, 0]]
    );
}
#[test]
fn disconnected_producer_cleans_active_notes() {
    let recording = Recording::default();
    let messages = recording.messages.clone();
    let (tx, rx) = mpsc::channel();
    tx.send(on(0)).unwrap();
    drop(tx);
    dispatch_loop(
        recording,
        rx,
        Arc::new(AtomicBool::new(true)),
        Instant::now(),
        MusicalClock { tempo: 120.0 },
        Duration::from_secs(10),
    )
    .unwrap();
    assert_eq!(
        *messages.lock().unwrap(),
        vec![vec![0x99, 42, 100], vec![0x89, 42, 0]]
    );
}
#[test]
fn send_failure_still_attempts_note_cleanup() {
    struct Failing {
        calls: Arc<Mutex<Vec<Vec<u8>>>>,
    }
    impl MidiOutput for Failing {
        fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.calls.lock().unwrap().push(bytes.to_vec());
            if bytes[0] & 0xf0 == 0x90 {
                Err("disconnected".into())
            } else {
                Ok(())
            }
        }
    }
    let calls = Arc::new(Mutex::new(vec![]));
    let (tx, rx) = mpsc::channel();
    tx.send(on(0)).unwrap();
    drop(tx);
    let result = dispatch_loop(
        Failing {
            calls: calls.clone(),
        },
        rx,
        Arc::new(AtomicBool::new(true)),
        Instant::now(),
        MusicalClock { tempo: 120.0 },
        Duration::from_secs(10),
    );
    assert!(result.is_err());
    assert_eq!(
        *calls.lock().unwrap(),
        vec![vec![0x99, 42, 100], vec![0x89, 42, 0]]
    );
}

#[test]
fn future_events_wait_for_deadline_and_preserve_note_order() {
    let recording = Recording::default();
    let messages = recording.messages.clone();
    let (tx, rx) = mpsc::channel();
    tx.send(on(0)).unwrap();
    tx.send(off(120)).unwrap();
    drop(tx);
    let origin = Instant::now() + Duration::from_millis(10);
    let clock = MusicalClock { tempo: 400.0 };
    dispatch_loop(
        recording,
        rx,
        Arc::new(AtomicBool::new(true)),
        origin,
        clock,
        Duration::from_secs(10),
    )
    .unwrap();
    assert!(Instant::now() >= origin + clock.time_at_tick(120));
    assert_eq!(
        *messages.lock().unwrap(),
        vec![vec![0x99, 42, 100], vec![0x89, 42, 0]]
    );
}

#[test]
fn cancellation_interrupts_waiting_for_a_future_note() {
    use std::sync::atomic::Ordering;
    let recording = Recording::default();
    let messages = recording.messages.clone();
    let (tx, rx) = mpsc::channel();
    tx.send(on(0)).unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let stop = running.clone();
    let worker = std::thread::spawn(move || {
        dispatch_loop(
            recording,
            rx,
            running,
            Instant::now() + Duration::from_secs(30),
            MusicalClock { tempo: 120.0 },
            Duration::from_secs(10),
        )
    });
    std::thread::sleep(Duration::from_millis(10));
    stop.store(false, Ordering::Relaxed);
    worker.join().unwrap().unwrap();
    assert!(messages.lock().unwrap().is_empty());
}

#[test]
fn stopping_releases_all_simultaneous_drum_voices() {
    let mut d = EventDispatcher::new(Recording::default());
    for note in [36, 38, 42, 46] {
        d.dispatch(
            &MidiEvent {
                stop_value: None,
                reset_value: None,
                parameter: false,
                tick: 0,
                bytes: [0x99, note, 100],
            },
            Duration::ZERO,
            Duration::ZERO,
        )
        .unwrap();
    }
    d.cleanup().unwrap();
    let messages = d.sink.messages.lock().unwrap();
    assert_eq!(messages.len(), 8);
    for note in [36, 38, 42, 46] {
        assert!(messages.contains(&vec![0x89, note, 0]));
    }
}

#[test]
fn clock_start_precedes_notes_and_cancel_sends_note_off_then_stop() {
    use std::sync::atomic::Ordering;
    let recording = Recording::default();
    let messages = recording.messages.clone();
    let running = Arc::new(AtomicBool::new(true));
    let (tx, rx) = mpsc::channel();
    tx.send(on(0)).unwrap();
    let control = running.clone();
    let worker = std::thread::spawn(move || {
        dispatch_loop_with_sync(
            recording,
            rx,
            control,
            Instant::now() + Duration::from_millis(20),
            MusicalClock { tempo: 132. },
            Duration::from_millis(20),
            sync::SyncOptions {
                enabled: true,
                end_tick: None,
            },
        )
    });
    let timeout = Instant::now() + Duration::from_secs(2);
    while !messages
        .lock()
        .unwrap()
        .iter()
        .any(|b| b == &[0x99, 42, 100])
        && Instant::now() < timeout
    {
        std::thread::sleep(Duration::from_millis(1));
    }
    running.store(false, Ordering::Relaxed);
    worker.join().unwrap().unwrap();
    drop(tx);
    let messages = messages.lock().unwrap();
    assert_eq!(
        &messages[..3],
        &[vec![0xfa], vec![0xf8], vec![0x99, 42, 100]]
    );
    assert_eq!(
        &messages[messages.len() - 2..],
        &[vec![0x89, 42, 0], vec![0xfc]]
    );
}
