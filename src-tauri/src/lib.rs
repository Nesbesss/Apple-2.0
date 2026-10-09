mod activity;
mod agents;
mod commands;
mod pty;
mod sound;
mod store;

use std::{collections::HashMap, sync::Mutex};
use tauri::Manager;

/// Backend state shared with every command. Never hold `store` and `ptys` at the same time.
pub struct AppState {
    pub store: Mutex<store::Store>,
    pub ptys: Mutex<HashMap<String, pty::Pty>>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let mut store = store::Store::load();
            if store.groups.is_empty() {
                store.groups.push(store::Group { name: "Group 1".into(), sessions: Vec::new() });
            }
            app.manage(AppState { store: Mutex::new(store), ptys: Mutex::new(HashMap::new()) });
            activity::spawn_watcher(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_store,
            commands::create_group,
            commands::rename_group,
            commands::delete_group,
            commands::create_session,
            commands::remove_session,
            commands::ensure_session,
            commands::restart_session,
            commands::stop_session,
            commands::write_pty,
            commands::resize_pty,
            commands::session_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Session Deck");
}
