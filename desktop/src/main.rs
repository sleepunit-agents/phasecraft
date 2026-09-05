#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use phasecraft::{
    authoring::project,
    playback::ports,
    player::{Player, Snapshot},
};
use serde::Serialize;
use std::{path::PathBuf, sync::Mutex};
use tauri::Manager;
mod updates;

#[derive(Default)]
struct AppState {
    player: Mutex<Player>,
    recent: Mutex<Vec<PathBuf>>,
    preferences: Mutex<Option<PathBuf>>,
    updates: updates::State,
}
#[derive(Serialize)]
struct Opened {
    project: project::ProjectInfo,
    selected: Option<PathBuf>,
    port: Option<String>,
    virtual_port: bool,
    send_clock: bool,
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
        send_clock: player.midi.send_clock,
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
    send_clock: bool,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let mut player = state.player.lock().map_err(|e| e.to_string())?;
    if state
        .updates
        .installing
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err("Update in progress".into());
    }
    player.play(port, virtual_port, silent, send_clock)
}
#[tauri::command(async)]
fn stop(state: tauri::State<AppState>) -> Result<(), String> {
    state.player.lock().map_err(|e| e.to_string())?.stop()
}
#[tauri::command]
fn snapshot(state: tauri::State<AppState>) -> Result<Snapshot, String> {
    Ok(state.player.lock().map_err(|e| e.to_string())?.poll())
}
#[tauri::command(async)]
fn setup_windows_midi(state: tauri::State<AppState>) -> Result<String, String> {
    let mut player = state.player.lock().map_err(|e| e.to_string())?;
    if player.poll().playing {
        return Err("Stop playback before setting up MIDI ports".into());
    }
    phasecraft::playback::windows_setup::setup()
}
#[tauri::command(async)]
fn get_midi_tools() -> Result<(), String> {
    #[cfg(windows)]
    {
        let windows = std::env::var_os("WINDIR").ok_or("Windows directory unavailable")?;
        std::process::Command::new(PathBuf::from(windows).join("explorer.exe"))
            .arg(phasecraft::playback::windows_setup::TOOLS_URL)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("Microsoft MIDI tools are for Windows".into())
    }
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            setup_windows_midi,
            get_midi_tools,
            open_project,
            new_project,
            select_composition,
            start,
            stop,
            snapshot,
            updates::check_update,
            updates::install_update
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                let _ = window.state::<AppState>().player.lock().unwrap().stop();
            }
        })
        .build(tauri::generate_context!())
        .expect("could not start Phasecraft Player")
        .run(|app, event| {
            // Application-menu Quit need not go through a window's close handler.
            if matches!(
                event,
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
            ) {
                let _ = app.state::<AppState>().player.lock().unwrap().stop();
            }
        });
}
