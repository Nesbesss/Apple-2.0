mod activity;
mod agents;
mod commands;
mod dictation;
mod pty;
mod sound;
mod store;
mod transcripts;

use std::{
    collections::HashMap,
    sync::{atomic::{AtomicBool, Ordering}, Mutex},
};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};

/// Backend state shared with every command. Never hold `store` and `ptys` at the same time.
pub struct AppState {
    pub store: Mutex<store::Store>,
    pub ptys: Mutex<HashMap<String, pty::Pty>>,
    pub dictation: dictation::Dictation,
}

/// Set once the user has agreed to quit while agents were still working.
pub static QUIT_CONFIRMED: AtomicBool = AtomicBool::new(false);

/// Quitting stops every agent, so when some are busy the frontend is asked first.
/// Returns true when the quit has been held back for that confirmation.
fn hold_quit_for_confirmation(app: &AppHandle) -> bool {
    if QUIT_CONFIRMED.load(Ordering::SeqCst) {
        return false;
    }
    let working = activity::working_count(&app.state::<AppState>());
    if working == 0 {
        return false;
    }
    let _ = app.emit("quit-requested", working);
    true
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let mut store = store::Store::load();
            if store.groups.is_empty() {
                store.groups.push(store::Group { name: "Group 1".into(), sessions: Vec::new() });
            }
            app.manage(AppState {
                store: Mutex::new(store),
                ptys: Mutex::new(HashMap::new()),
                dictation: Default::default(),
            });
            if let Err(err) = dictation::warm(app.handle(), &app.state::<AppState>()) {
                eprintln!("dictation: {err}");
            }
            activity::spawn_watcher(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if hold_quit_for_confirmation(window.app_handle()) {
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_store,
            commands::create_group,
            commands::rename_group,
            commands::delete_group,
            commands::create_session,
            commands::remove_session,
            commands::rename_session,
            commands::move_session,
            commands::reveal_session_folder,
            commands::copy_text,
            commands::notify,
            commands::set_alerts,
            commands::play_sound,
            commands::save_dropped_file,
            commands::quit_app,
            commands::ensure_session,
            commands::restart_session,
            commands::stop_session,
            commands::write_pty,
            commands::resize_pty,
            commands::session_status,
            commands::list_transcripts,
            commands::resume_session,
            dictation::dictation_start,
            dictation::dictation_stop,
            dictation::dictation_cancel,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Nesa")
        .run(|app, event| {
            // Cmd+Q and the app menu end up here, not at the window.
            if let RunEvent::ExitRequested { api, .. } = event {
                if hold_quit_for_confirmation(app) {
                    api.prevent_exit();
                }
            }
        });
}
