#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use phasecraft::{
    authoring::project,
    playback::ports,
    player::{Player, Snapshot},
};
use serde::Serialize;
use std::{path::PathBuf, sync::Mutex};
use tauri::Manager;

#[derive(Default)]
struct AppState {
    player: Mutex<Player>,
    recent: Mutex<Vec<PathBuf>>,
    preferences: Mutex<Option<PathBuf>>,
}
#[derive(Serialize)]
struct Opened {
    project: project::ProjectInfo,
    selected: Option<PathBuf>,
    port: Option<String>,
    virtual_port: bool,
}
#[derive(Serialize)]
struct Initial {
    recent: Vec<PathBuf>,
    version: phasecraft::update::Version,
}
#[tauri::command]
fn initial(state: tauri::State<AppState>) -> Result<Initial, String> {
    Ok(Initial {
        recent: state.recent.lock().map_err(|e| e.to_string())?.clone(),
        version: phasecraft::update::version(),
    })
}
#[tauri::command(async)]
fn destinations() -> Result<Vec<String>, String> {
    ports::list()
}
#[tauri::command(async)]
fn open_project(path: PathBuf, state: tauri::State<AppState>) -> Result<Opened, String> {
    let mut player = state.player.lock().map_err(|e| e.to_string())?;
    let project = player.open(&path)?;
    let mut recent = state.recent.lock().map_err(|e| e.to_string())?;
    recent.retain(|p| p != &project.path);
    recent.insert(0, project.path.clone());
    recent.truncate(8);
    if let Some(path) = state
        .preferences
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
    {
        // Preferences are expendable; a disk error must not prevent playing a project.
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec(&*recent) {
            let _ = std::fs::write(path, bytes);
        }
    }
    Ok(Opened {
        project,
        selected: player.selected.clone(),
        port: player.midi.port.clone(),
        virtual_port: player.midi.virtual_port,
    })
}
#[tauri::command(async)]
fn new_project(path: PathBuf, state: tauri::State<AppState>) -> Result<Opened, String> {
    project::create(&path)?;
    open_project(path, state)
}
#[tauri::command(async)]
fn select_composition(path: PathBuf, state: tauri::State<AppState>) -> Result<(), String> {
    state
        .player
        .lock()
        .map_err(|e| e.to_string())?
        .select(&path)
}
#[tauri::command(async)]
fn start(
    port: Option<String>,
    virtual_port: bool,
    silent: bool,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    state
        .player
        .lock()
        .map_err(|e| e.to_string())?
        .play(port, virtual_port, silent)
}
#[tauri::command(async)]
fn stop(state: tauri::State<AppState>) -> Result<(), String> {
    state.player.lock().map_err(|e| e.to_string())?.stop()
}
#[tauri::command]
fn snapshot(state: tauri::State<AppState>) -> Result<Snapshot, String> {
    Ok(state.player.lock().map_err(|e| e.to_string())?.poll())
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            let state = app.state::<AppState>();
            if let Ok(path) = app.path().app_config_dir().map(|p| p.join("recent.json")) {
                if let Ok(bytes) = std::fs::read(&path)
                    && let Ok(recent) = serde_json::from_slice::<Vec<PathBuf>>(&bytes)
                {
                    *state.recent.lock().unwrap() = recent.into_iter().take(8).collect();
                }
                *state.preferences.lock().unwrap() = Some(path);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            initial,
            destinations,
            open_project,
            new_project,
            select_composition,
            start,
            stop,
            snapshot
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                let _ = window.state::<AppState>().player.lock().unwrap().stop();
            }
        })
        .run(tauri::generate_context!())
        .expect("could not start Phasecraft Player");
}
