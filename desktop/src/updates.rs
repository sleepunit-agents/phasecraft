//! Desktop updates have their own signed feed, separate from CLI executables.
use crate::AppState;
use serde::Serialize;
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::{Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
pub struct State {
    pending: Mutex<Option<Update>>,
    pub installing: AtomicBool,
}
#[derive(Serialize)]
pub struct Status {
    commit: Option<String>,
    supported: bool,
}
fn different_commit(remote: &str, local: &str) -> bool {
    remote.len() == 40
        && remote.bytes().all(|b| b.is_ascii_hexdigit())
        && local.len() == 40
        && remote != local
}
#[tauri::command]
pub async fn check_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Status, String> {
    // Package managers own .deb installations. Only AppImage supports in-app Linux replacement.
    let supported = !cfg!(target_os = "linux") || app.env().appimage.is_some();
    if !supported || cfg!(debug_assertions) {
        return Ok(Status {
            commit: None,
            supported: false,
        });
    }
    let update = app
        .updater_builder()
        .timeout(Duration::from_secs(120))
        .version_comparator(|_, release| {
            different_commit(release.version.build.as_str(), phasecraft::update::COMMIT)
        })
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let commit = update
        .as_ref()
        .and_then(|u| u.version.split_once('+').map(|(_, c)| c.to_owned()));
    *state.updates.pending.lock().map_err(|e| e.to_string())? = update;
    Ok(Status { commit, supported })
}
struct Installing<'a>(&'a AtomicBool);
impl Drop for Installing<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
#[tauri::command]
pub async fn install_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if state.updates.installing.swap(true, Ordering::SeqCst) {
        return Err("Update already in progress".into());
    }
    let _guard = Installing(&state.updates.installing);
    let update = state
        .updates
        .pending
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("Check for an update first")?;
    // Acknowledge and join the MIDI worker before any Windows installer can exit us.
    state.player.lock().map_err(|e| e.to_string())?.stop()?;
    let mut received = 0_u64;
    let bytes = update
        .download(
            |chunk, total| {
                received += chunk as u64;
                let _ = app.emit(
                    "update-progress",
                    serde_json::json!({"received":received,"total":total}),
                );
            },
            || {},
        )
        .await
        .map_err(|e| format!("Update download/verification failed: {e}. Check again to retry."))?;
    // Signature verification completed; install away from the webview/transport threads.
    tauri::async_runtime::spawn_blocking(move || update.install(bytes))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Could not install update: {e}"))?;
    app.restart();
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rolling_builds_compare_full_commits_not_semver_or_short_prefixes() {
        let a = "a".repeat(40);
        assert!(!different_commit(&a, &a));
        assert!(different_commit(&format!("{}b", "a".repeat(39)), &a));
        assert!(!different_commit("dev", &a));
        assert!(!different_commit(&a, "unknown"));
    }
}
