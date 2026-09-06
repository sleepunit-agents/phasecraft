#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use phasecraft::{
    authoring::project,
    playback::ports,
    player::{Player, Snapshot},
};
use serde::Serialize;
use std::{path::PathBuf, sync::Mutex};
use tauri::Manager;
mod settings;
mod updates;

#[derive(Default)]
struct AppState {
    player: Mutex<Player>,
    controller: Mutex<Option<phasecraft::playback::controller::Connection>>,
    recent: Mutex<Vec<PathBuf>>,
    preferences: Mutex<Option<PathBuf>>,
    updates: updates::State,
    settings: Mutex<settings::Settings>,
}
#[derive(Serialize)]
struct Opened {
    project: project::ProjectInfo,
    selected: Option<PathBuf>,
    port: Option<String>,
    virtual_port: bool,
    send_clock: bool,
    silent: bool,
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
    let routing = state
        .settings
        .lock()
        .map_err(|e| e.to_string())?
        .projects
        .get(&project.path)
        .cloned()
        .unwrap_or(settings::Routing {
            port: player.midi.port.clone(),
            virtual_port: player.midi.virtual_port,
            send_clock: player.midi.send_clock,
            silent: false,
        });
    Ok(Opened {
        project,
        selected: player.selected.clone(),
        port: routing.port,
        virtual_port: routing.virtual_port,
        send_clock: routing.send_clock,
        silent: routing.silent,
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
fn close_project(state: tauri::State<AppState>) -> Result<(), String> {
    state.player.lock().map_err(|e| e.to_string())?.close()
}
#[tauri::command(async)]
fn save_settings(routing: settings::Routing, state: tauri::State<AppState>) -> Result<(), String> {
    let mut player = state.player.lock().map_err(|e| e.to_string())?;
    if player.poll().playing {
        return Err("Stop playback before changing MIDI settings".into());
    }
    let project = player
        .project
        .as_ref()
        .ok_or("Open a project first")?
        .path
        .clone();
    let path = state
        .preferences
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .ok_or("Settings directory unavailable")?
        .with_file_name("player-settings.json");
    state
        .settings
        .lock()
        .map_err(|e| e.to_string())?
        .save(&path, project, routing)
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
#[tauri::command(async)]
fn controller_inputs() -> Result<Vec<String>, String> {
    phasecraft::playback::controller::inputs()
}
#[tauri::command(async)]
fn controller_connect(
    input: String,
    output: String,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let live = state.player.lock().map_err(|e| e.to_string())?.live.clone();
    let mut connection = state.controller.lock().map_err(|e| e.to_string())?;
    *connection = None;
    *connection = Some(phasecraft::playback::controller::connect(
        input, output, live,
    )?);
    Ok(())
}
#[tauri::command(async)]
fn controller_disconnect(state: tauri::State<AppState>) -> Result<(), String> {
    *state.controller.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}
#[tauri::command]
fn controller_status(
    state: tauri::State<AppState>,
) -> Result<phasecraft::playback::controller::Status, String> {
    let mut status = state
        .controller
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .map(|c| c.status())
        .unwrap_or_default();
    status.view = Some(
        state
            .player
            .lock()
            .map_err(|e| e.to_string())?
            .live
            .lock()
            .map_err(|e| e.to_string())?
            .view("kick"),
    );
    Ok(status)
}
#[tauri::command]
fn controller_reset(state: tauri::State<AppState>) -> Result<(), String> {
    state
        .player
        .lock()
        .map_err(|e| e.to_string())?
        .live
        .lock()
        .map_err(|e| e.to_string())?
        .reset();
    Ok(())
}
#[tauri::command]
fn window_control(window: tauri::WebviewWindow, action: &str) -> Result<(), String> {
    match action {
        "minimize" => window.minimize(),
        "maximize" => {
            if window.is_maximized().map_err(|e| e.to_string())? {
                window.unmaximize()
            } else {
                window.maximize()
            }
        }
        "close" => window.close(),
        "drag" => window.start_dragging(),
        _ => return Err("Unknown window action".into()),
    }
    .map_err(|e| e.to_string())
}
#[tauri::command]
fn snapshot(state: tauri::State<AppState>) -> Result<Snapshot, String> {
    Ok(state.player.lock().map_err(|e| e.to_string())?.poll())
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
                *state.settings.lock().unwrap() =
                    settings::Settings::load(&path.with_file_name("player-settings.json"));
                *state.preferences.lock().unwrap() = Some(path);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            initial,
            controller_inputs,
            controller_connect,
            controller_disconnect,
            controller_status,
            controller_reset,
            window_control,
            destinations,
            open_project,
            new_project,
            select_composition,
            close_project,
            save_settings,
            start,
            stop,
            snapshot,
            updates::check_update,
            updates::install_update
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                let state = window.state::<AppState>();
                *state.controller.lock().unwrap() = None;
                let _ = state.player.lock().unwrap().stop();
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
                let state = app.state::<AppState>();
                *state.controller.lock().unwrap() = None;
                let _ = state.player.lock().unwrap().stop();
            }
        });
}
