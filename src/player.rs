//! Desktop-independent player state. The UI reads due snapshots, never scheduled-ahead hits.
use crate::{
    authoring::project::{self, MidiSettings, ProjectInfo},
    music::{
        Composition,
        resolve::{StepTrace, resolve_step},
    },
    playback::{
        MidiOutput, SilentOutput, ports,
        transport::{PlayOptions, PlaybackFrame, run_controlled},
    },
};
use serde::Serialize;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Serialize)]
pub struct Snapshot {
    pub playing: bool,
    pub seed_label: Option<String>,
    pub step: Option<u64>,
    pub progress: f64,
    pub composition: Option<Arc<Composition>>,
    pub traces: Vec<StepTrace>,
    pub history: Vec<HistoryStep>,
    pub error: Option<String>,
    pub reload_error: Option<String>,
}
#[derive(Clone, Serialize)]
pub struct HistoryStep {
    pub step: u64,
    pub parts: Vec<Hit>,
}
#[derive(Clone, Serialize)]
pub struct Hit {
    pub id: String,
    pub eligible: bool,
    pub fired: bool,
    pub accented: bool,
}
struct Session {
    running: Arc<AtomicBool>,
    worker: JoinHandle<Result<(), String>>,
    receiver: mpsc::Receiver<PlaybackFrame>,
}
#[derive(Default)]
pub struct Player {
    pub project: Option<ProjectInfo>,
    pub selected: Option<PathBuf>,
    pub midi: MidiSettings,
    composition: Option<Arc<Composition>>,
    session: Option<Session>,
    pending: VecDeque<PlaybackFrame>,
    current: Option<PlaybackFrame>,
    history: VecDeque<HistoryStep>,
    error: Option<String>,
}
impl Player {
    pub fn open(&mut self, path: &Path) -> Result<ProjectInfo, String> {
        let info = project::describe(path)?;
        let loaded = project::load(&info.default)?;
        self.stop()?;
        self.project = Some(info.clone());
        self.selected = Some(info.default.clone());
        self.composition = Some(Arc::new(loaded.composition));
        self.midi = loaded.midi;
        self.reset_view();
        Ok(info)
    }
    pub fn select(&mut self, path: &Path) -> Result<(), String> {
        let info = self.project.as_ref().ok_or("open a project first")?;
        if !info.compositions.iter().any(|p| p == path) {
            return Err("composition is not in this project".into());
        }
        let loaded = project::load(path)?;
        self.stop()?;
        self.selected = Some(path.to_owned());
        self.composition = Some(Arc::new(loaded.composition));
        self.midi = loaded.midi;
        self.reset_view();
        Ok(())
    }
    fn reset_view(&mut self) {
        self.current = None;
        self.pending.clear();
        self.history.clear();
        self.error = None;
    }
    pub fn play(
        &mut self,
        port: Option<String>,
        virtual_port: bool,
        silent: bool,
        send_clock: bool,
    ) -> Result<(), String> {
        let sink: Box<dyn MidiOutput> = if silent {
            Box::new(SilentOutput)
        } else {
            Box::new(ports::open_output(port, virtual_port)?)
        };
        self.start_with_clock(sink, Some(send_clock))
    }
    pub fn start<S: MidiOutput + 'static>(&mut self, sink: S) -> Result<(), String> {
        self.start_with_clock(sink, None)
    }
    fn start_with_clock<S: MidiOutput + 'static>(
        &mut self,
        sink: S,
        send_clock: Option<bool>,
    ) -> Result<(), String> {
        self.stop()?;
        let file = self.selected.clone().ok_or("open a project first")?;
        let loaded = project::load(&file)?;
        self.midi = loaded.midi;
        if let Some(send_clock) = send_clock {
            self.midi.send_clock = send_clock;
        }
        self.composition = Some(Arc::new(loaded.composition.clone()));
        self.reset_view();
        let running = Arc::new(AtomicBool::new(true));
        let control = running.clone();
        let (sender, receiver) = mpsc::sync_channel(128);
        let options = PlayOptions {
            file,
            steps: None,
            watch: true,
            trace: false,
            send_clock: self.midi.send_clock,
            lookahead: Duration::from_millis(self.midi.lookahead_ms),
        };
        let worker = std::thread::spawn(move || {
            let result = run_controlled(
                loaded.composition,
                sink,
                options,
                control.clone(),
                Some(sender),
            );
            control.store(false, Ordering::Relaxed);
            result
        });
        self.session = Some(Session {
            running,
            worker,
            receiver,
        });
        Ok(())
    }
    pub fn stop(&mut self) -> Result<(), String> {
        if let Some(session) = self.session.take() {
            session.running.store(false, Ordering::Relaxed);
            let result = session
                .worker
                .join()
                .map_err(|_| "playback worker panicked".to_string())?;
            if let Err(e) = &result {
                self.error = Some(e.clone());
            }
            self.pending.clear();
            return result;
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Snapshot {
        self.poll_at(Instant::now())
    }
    fn poll_at(&mut self, now: Instant) -> Snapshot {
        if let Some(session) = &self.session {
            while let Ok(frame) = session.receiver.try_recv() {
                if self.pending.len() == 256 {
                    self.pending.pop_front();
                }
                self.pending.push_back(frame);
            }
        }
        while self.pending.front().is_some_and(|f| f.deadline <= now) {
            let frame = self.pending.pop_front().unwrap();
            let parts = frame
                .traces
                .iter()
                .map(|t| Hit {
                    id: t.part.clone(),
                    eligible: t.trigger.rhythm.active(),
                    fired: t.event.is_some(),
                    accented: t.event.as_ref().is_some_and(|e| e.accent.active),
                })
                .collect();
            self.history.push_back(HistoryStep {
                step: frame.step,
                parts,
            });
            if self.history.len() > 32 {
                self.history.pop_front();
            }
            self.composition = Some(frame.composition.clone());
            self.current = Some(frame);
        }
        if self
            .session
            .as_ref()
            .is_some_and(|s| s.worker.is_finished())
        {
            let _ = self.stop();
        }
        let playing = self
            .session
            .as_ref()
            .is_some_and(|s| s.running.load(Ordering::Relaxed));
        let step = self.current.as_ref().map(|f| f.step);
        let progress = if playing {
            self.current
                .as_ref()
                .map(|f| {
                    now.saturating_duration_since(f.deadline).as_secs_f64()
                        / (60.0 / f.composition.tempo / 4.0)
                })
                .unwrap_or(0.0)
                .min(1.0)
        } else {
            0.0
        };
        // Resolving here is outside both playback threads and uses the exact visible model.
        let traces = self
            .composition
            .as_ref()
            .map(|c| resolve_step(c, step.unwrap_or(0)).0)
            .unwrap_or_default();
        Snapshot {
            seed_label: self.composition.as_ref().map(|c| c.seed.to_string()),
            playing,
            step,
            progress,
            composition: self.composition.clone(),
            traces,
            history: self.history.iter().cloned().collect(),
            error: self.error.clone(),
            reload_error: self.current.as_ref().and_then(|f| f.reload_error.clone()),
        }
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_waits_for_deadline_and_retains_independent_cycle_phases() {
        let c =
            Arc::new(Composition::parse(include_str!("../examples/quickstart/hat.toml")).unwrap());
        let now = Instant::now();
        let mut player = Player::default();
        player.composition = Some(c.clone());
        player.pending.push_back(PlaybackFrame {
            deadline: now + Duration::from_secs(1),
            step: 17,
            composition: c.clone(),
            traces: resolve_step(&c, 17).0,
            reload_error: None,
        });
        assert_eq!(player.poll_at(now).step, None);
        assert!(player.history.is_empty());
        let due = player.poll_at(now + Duration::from_secs(1));
        assert_eq!(due.step, Some(17));
        assert_eq!(
            serde_json::to_string(&due.traces).unwrap(),
            serde_json::to_string(&resolve_step(&c, 17).0).unwrap()
        );
    }
    #[test]
    fn repeat_start_stop_releases_notes_without_a_global_signal_handler() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("project");
        project::create(&path).unwrap();
        let notes = Arc::new(std::sync::Mutex::new(Vec::new()));
        struct Sink(Arc<std::sync::Mutex<Vec<Vec<u8>>>>);
        impl MidiOutput for Sink {
            fn send(&mut self, b: &[u8]) -> Result<(), String> {
                self.0.lock().unwrap().push(b.to_vec());
                Ok(())
            }
        }
        let mut player = Player::default();
        player.open(&path).unwrap();
        for _ in 0..3 {
            let before = notes.lock().unwrap().len();
            player.start(Sink(notes.clone())).unwrap();
            let limit = Instant::now() + Duration::from_secs(2);
            while notes.lock().unwrap().len() == before && Instant::now() < limit {
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(notes.lock().unwrap().len() > before);
            player.stop().unwrap();
            assert!(!player.poll().playing);
        }
        let mut active = std::collections::BTreeSet::new();
        for event in notes.lock().unwrap().iter() {
            let key = (event[0] & 15, event[1]);
            if event[0] & 0xf0 == 0x90 {
                active.insert(key);
            } else {
                active.remove(&key);
            }
        }
        assert!(active.is_empty());
        assert!(!notes.lock().unwrap().is_empty());
    }
    #[test]
    fn full_visual_channel_cannot_block_the_transport() {
        let c = Composition::parse(include_str!("../examples/quickstart/hat.toml")).unwrap();
        let running = Arc::new(AtomicBool::new(true));
        let (sender, _receiver) = mpsc::sync_channel(0);
        let options = PlayOptions {
            file: PathBuf::new(),
            steps: Some(2),
            watch: false,
            trace: false,
            send_clock: false,
            lookahead: Duration::from_millis(10),
        };
        let began = Instant::now();
        run_controlled(c, SilentOutput, options, running, Some(sender)).unwrap();
        assert!(began.elapsed() < Duration::from_secs(2));
    }
}
