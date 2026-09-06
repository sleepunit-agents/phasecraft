//! Disk reads, expansion, validation and compilation never run on the MIDI producer.
use crate::music::{Composition, resolve::Compiled};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
pub(super) struct Candidate {
    pub composition: Composition,
    pub compiled: Compiled,
    pub source: String,
}
pub(super) struct Watcher {
    latest: Arc<Mutex<Option<Result<Candidate, String>>>>,
    running: Arc<AtomicBool>,
}
impl Watcher {
    pub fn new(file: PathBuf, baseline: Composition) -> Self {
        Self::spawn(move || {
            let next = Composition::read(&file)?;
            if next.tempo != baseline.tempo || next.phrase_bars != baseline.phrase_bars {
                return Err("restart playback to change tempo or phrase_bars".into());
            }
            if !baseline.same_arrangement_layout(&next)
                || baseline.parts.len() != next.parts.len()
                || baseline.parts.iter().zip(&next.parts).any(|(a, b)| {
                    a.id != b.id || a.output != b.output || a.subdivision != b.subdivision
                })
            {
                return Err(
                    "restart playback to change arrangement layout, subdivision or routing".into(),
                );
            }
            Ok(next)
        })
    }
    fn spawn(mut load: impl FnMut() -> Result<Composition, String> + Send + 'static) -> Self {
        let latest = Arc::new(Mutex::new(None));
        let running = Arc::new(AtomicBool::new(true));
        let slot = latest.clone();
        let active = running.clone();
        std::thread::spawn(move || {
            let mut last = None;
            while active.load(Ordering::Relaxed) {
                let result = load().and_then(|composition| {
                    let source = serde_json::to_string(&composition).map_err(|e| e.to_string())?;
                    Ok((composition, source))
                });
                let fingerprint = result
                    .as_ref()
                    .map(|(_, s)| s.clone())
                    .map_err(Clone::clone);
                if last.as_ref() != Some(&fingerprint) {
                    last = Some(fingerprint);
                    let candidate = result.map(|(composition, source)| Candidate {
                        compiled: Compiled::new(&composition),
                        composition,
                        source,
                    });
                    *slot.lock().unwrap() = Some(candidate);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        Self { latest, running }
    }
    pub fn take(&self) -> Option<Result<Candidate, String>> {
        self.latest.try_lock().ok()?.take()
    }
}
impl Drop for Watcher {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stalled_loader_does_not_block_polling_or_stop() {
        let (entered, ready) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let watcher = Watcher::spawn(move || {
            entered.send(()).unwrap();
            wait.recv().unwrap();
            Err("slow disk".into())
        });
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        // Loader is provably blocked until after poll and shutdown complete.
        assert!(watcher.take().is_none());
        drop(watcher);
        release.send(()).unwrap();
    }
}
