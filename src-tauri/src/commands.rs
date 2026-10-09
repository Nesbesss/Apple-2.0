use crate::{
    activity, pty, sound, transcripts,
    store::{default_name, Group, Kind, Session, Store},
    AppState,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::Ordering,
};
use tauri::{AppHandle, State};
use uuid::Uuid;

type CmdResult<T> = Result<T, String>;

#[derive(Serialize)]
pub struct SessionStatus {
    running: bool,
    exited: bool,
    working: bool,
}

fn save(store: &Store) -> CmdResult<()> {
    store.save().map_err(|e| format!("Could not save groups: {e}"))
}

/// Applies `edit` to a copy, saves it, and only then replaces the live store. A failed save
/// leaves both memory and groups.json unchanged.
fn edit_store(state: &AppState, edit: impl FnOnce(&mut Store) -> CmdResult<()>) -> CmdResult<Store> {
    let mut store = state.store.lock().unwrap();
    let mut next = store.clone();
    edit(&mut next)?;
    save(&next)?;
    *store = next.clone();
    Ok(next)
}

fn find_session(state: &AppState, id: &str) -> CmdResult<Session> {
    state
        .store
        .lock()
        .unwrap()
        .groups
        .iter()
        .flat_map(|g| &g.sessions)
        .find(|s| s.id == id)
        .cloned()
        .ok_or_else(|| "That session no longer exists".to_string())
}

fn expand_home(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(path),
    }
}

#[tauri::command]
pub fn load_store(state: State<'_, AppState>) -> Store {
    state.store.lock().unwrap().clone()
}

#[tauri::command]
pub fn create_group(name: String, state: State<'_, AppState>) -> CmdResult<Store> {
    let mut store = state.store.lock().unwrap();
    let trimmed = name.trim();
    let name = if trimmed.is_empty() {
        format!("Group {}", store.groups.len() + 1)
    } else {
        trimmed.to_string()
    };
    store.groups.push(Group { name, sessions: Vec::new() });
    save(&store)?;
    Ok(store.clone())
}

#[tauri::command]
pub fn rename_group(index: usize, name: String, state: State<'_, AppState>) -> CmdResult<Store> {
    let mut store = state.store.lock().unwrap();
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Group name cannot be empty".into());
    }
    let group = store.groups.get_mut(index).ok_or("That group no longer exists")?;
    group.name = trimmed.to_string();
    save(&store)?;
    Ok(store.clone())
}

#[tauri::command]
pub fn delete_group(index: usize, state: State<'_, AppState>) -> CmdResult<Store> {
    let mut store = state.store.lock().unwrap();
    if store.groups.len() <= 1 {
        return Err("The last group cannot be deleted".into());
    }
    if index >= store.groups.len() {
        return Err("That group no longer exists".into());
    }
    let removed = store.groups.remove(index);
    save(&store)?;
    let snapshot = store.clone();
    drop(store);

    let mut ptys = state.ptys.lock().unwrap();
    for session in removed.sessions {
        if let Some(mut pty) = ptys.remove(&session.id) {
            pty.stop();
        }
    }
    Ok(snapshot)
}

#[tauri::command]
pub fn create_session(
    group_index: usize,
    kind: Kind,
    name: String,
    cwd: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Session> {
    let path = expand_home(cwd.trim());
    let cwd = fs::canonicalize(&path).map_err(|_| format!("No such folder: {}", path.display()))?;
    if !cwd.is_dir() {
        return Err(format!("Not a folder: {}", cwd.display()));
    }

    let id = Uuid::new_v4().to_string();
    let name = {
        let store = state.store.lock().unwrap();
        let group = store.groups.get(group_index).ok_or("That group no longer exists")?;
        match name.trim() {
            "" => {
                // The folder name says far more than "Claude 3", and it is what people look for.
                let folder = cwd
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_else(|| kind.label().to_string());
                let taken: Vec<&str> = group.sessions.iter().map(|s| s.name.as_str()).collect();
                default_name(&folder, &taken)
            }
            typed => typed.to_string(),
        }
    };
    // Claude takes a caller-chosen id, so the same id is used to launch and to resume.
    let agent_id = (kind == Kind::Claude).then(|| id.clone());
    start_session(&app, &state, group_index, Session { id, kind, name, cwd, agent_id })
}

/// Starts a session's agent and files it under group `group_index`. New and resumed sessions both go through here.
fn start_session(app: &AppHandle, state: &AppState, group_index: usize, session: Session) -> CmdResult<Session> {
    let mut ptys = state.ptys.lock().unwrap();
    let pty = pty::spawn(app, &session)?;
    ptys.insert(session.id.clone(), pty);
    drop(ptys);

    let mut store = state.store.lock().unwrap();
    let group = store.groups.get_mut(group_index).ok_or("That group no longer exists")?;
    group.sessions.push(session.clone());
    save(&store)?;
    Ok(session)
}

/// A past session found on disk, with what the deck already knows about it.
#[derive(Serialize)]
pub struct ResumableSession {
    #[serde(flatten)]
    transcript: transcripts::Transcript,
    /// The deck session that already holds this transcript, if any.
    deck_session: Option<String>,
    /// A live Claude process has this session open, here or in another terminal. Resuming it would run it twice.
    running: bool,
}

/// Past Claude and Codex sessions, newest first. Reading transcripts is file I/O, so it runs off the UI thread.
#[tauri::command]
pub async fn list_transcripts(state: State<'_, AppState>) -> CmdResult<Vec<ResumableSession>> {
    let deck: HashMap<String, String> = state
        .store
        .lock()
        .unwrap()
        .groups
        .iter()
        .flat_map(|g| &g.sessions)
        .filter_map(|s| s.agent_id.clone().map(|agent| (agent, s.id.clone())))
        .collect();
    let running = activity::claude_running_ids();
    let found = tauri::async_runtime::spawn_blocking(|| transcripts::recent(transcripts::LIST_LIMIT))
        .await
        .map_err(|e| e.to_string())?;
    Ok(found
        .into_iter()
        .map(|transcript| ResumableSession {
            deck_session: deck.get(&transcript.id).cloned(),
            running: transcript.kind == Kind::Claude && running.contains(&transcript.id),
            transcript,
        })
        .collect())
}

/// Resumes a past session in group `group_index`. A transcript the deck already holds is returned as it
/// is, so one conversation never runs twice.
#[tauri::command]
pub fn resume_session(
    group_index: usize,
    kind: Kind,
    agent_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Session> {
    if !transcripts::is_valid_id(&agent_id) {
        return Err("That session id is not valid".into());
    }
    let taken: Vec<String> = {
        let store = state.store.lock().unwrap();
        if let Some(existing) = store
            .groups
            .iter()
            .flat_map(|g| &g.sessions)
            .find(|s| s.agent_id.as_deref() == Some(agent_id.as_str()))
        {
            return Ok(existing.clone());
        }
        let group = store.groups.get(group_index).ok_or("That group no longer exists")?;
        group.sessions.iter().map(|s| s.name.clone()).collect()
    };
    if kind == Kind::Claude && activity::claude_running_ids().contains(&agent_id) {
        return Err("This session is open in another terminal. Close it there first.".into());
    }
    let transcript = transcripts::find(kind, &agent_id).ok_or("That session is no longer on disk")?;

    // The transcript's own title names the session, and the folder name is the fallback.
    let folder = transcript
        .cwd
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| kind.label().to_string());
    let base = if transcript.title.is_empty() { folder } else { transcript.title };
    let taken: Vec<&str> = taken.iter().map(String::as_str).collect();
    let name = default_name(&base, &taken);
    let session = Session { id: Uuid::new_v4().to_string(), kind, name, cwd: transcript.cwd, agent_id: Some(agent_id) };
    start_session(&app, &state, group_index, session)
}

#[tauri::command]
pub fn remove_session(id: String, state: State<'_, AppState>) -> CmdResult<Store> {
    if let Some(mut pty) = state.ptys.lock().unwrap().remove(&id) {
        pty.stop();
    }
    let mut store = state.store.lock().unwrap();
    for group in &mut store.groups {
        group.sessions.retain(|s| s.id != id);
    }
    save(&store)?;
    Ok(store.clone())
}

/// Shows the session's folder in Finder. Only folders the deck already knows about can be shown.
#[tauri::command]
pub fn reveal_session_folder(id: String, state: State<'_, AppState>) -> CmdResult<()> {
    let session = find_session(&state, &id)?;
    let status = Command::new("open")
        .arg("-R")
        .arg(&session.cwd)
        .status()
        .map_err(|e| format!("Could not open Finder: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Finder could not show that folder".into())
    }
}

#[tauri::command]
pub fn rename_session(id: String, name: String, state: State<'_, AppState>) -> CmdResult<Store> {
    edit_store(&state, |store| store.rename_session(&id, &name))
}

/// Moves a session to another group. Its agent keeps running, so nothing is restarted.
#[tauri::command]
pub fn move_session(id: String, group_index: usize, state: State<'_, AppState>) -> CmdResult<Store> {
    edit_store(&state, |store| store.move_session(&id, group_index))
}

/// Puts text on the clipboard. Goes through pbcopy because WKWebView clipboard access
/// depends on the page's gesture state.
#[tauri::command]
pub fn copy_text(text: String) -> CmdResult<()> {
    let copy_error = |e: std::io::Error| format!("Could not copy: {e}");
    let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn().map_err(copy_error)?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).map_err(copy_error)?;
    }
    child.wait().map_err(copy_error)?;
    Ok(())
}

/// Shows a macOS notification. Title and body are passed as arguments, never pasted into the
/// script, so a session name cannot change what runs.
#[tauri::command]
pub fn notify(title: String, body: String) {
    if !sound::alerts_enabled() {
        return;
    }
    std::thread::spawn(move || {
        let _ = Command::new("osascript")
            .args(["-e", "on run argv", "-e", "display notification (item 2 of argv) with title (item 1 of argv)", "-e", "end run"])
            .arg(&title)
            .arg(&body)
            .status();
    });
}

#[tauri::command]
pub fn set_alerts(enabled: bool) {
    sound::set_alerts_enabled(enabled);
}

/// Quits after the user has confirmed while agents were still working.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    crate::QUIT_CONFIRMED.store(true, Ordering::SeqCst);
    app.exit(0);
}

/// Starts the agent if it has never been started in this run. An exited session is left as is,
/// so showing a card never restarts a process on its own.
///
/// Lock order: the session is looked up before `ptys` is taken, so `ptys` is never held while
/// `store` is locked. The check, spawn and insert happen under one `ptys` guard, so two calls
/// for the same id cannot both start a process.
#[tauri::command]
pub fn ensure_session(id: String, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let session = find_session(&state, &id)?;
    let mut ptys = state.ptys.lock().unwrap();
    if ptys.contains_key(&id) {
        return Ok(());
    }
    let pty = pty::spawn(&app, &session)?;
    ptys.insert(id, pty);
    Ok(())
}

#[tauri::command]
pub fn restart_session(id: String, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let session = find_session(&state, &id)?;
    let mut ptys = state.ptys.lock().unwrap();
    if let Some(mut old) = ptys.remove(&id) {
        old.stop();
    }
    let pty = pty::spawn(&app, &session)?;
    ptys.insert(id, pty);
    Ok(())
}

#[tauri::command]
pub fn stop_session(id: String, state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(pty) = state.ptys.lock().unwrap().get_mut(&id) {
        pty.stop();
    }
    Ok(())
}

/// Keystrokes for a finished session are dropped quietly, so typing never shows errors.
#[tauri::command]
pub fn write_pty(id: String, data: String, state: State<'_, AppState>) -> CmdResult<()> {
    let mut ptys = state.ptys.lock().unwrap();
    match ptys.get_mut(&id) {
        Some(pty) if pty.is_running() => pty.write(data.as_bytes()),
        _ => Ok(()),
    }
}

#[tauri::command]
pub fn resize_pty(id: String, cols: u16, rows: u16, state: State<'_, AppState>) -> CmdResult<()> {
    let ptys = state.ptys.lock().unwrap();
    match ptys.get(&id) {
        Some(pty) if pty.is_running() => pty.resize(cols.max(2), rows.max(2)),
        _ => Ok(()),
    }
}

#[tauri::command]
pub fn session_status(ids: Vec<String>, state: State<'_, AppState>) -> HashMap<String, SessionStatus> {
    let sessions: HashMap<String, Session> = state
        .store
        .lock()
        .unwrap()
        .groups
        .iter()
        .flat_map(|g| g.sessions.clone())
        .map(|s| (s.id.clone(), s))
        .collect();
    let busy = activity::claude_busy_ids();
    let ptys = state.ptys.lock().unwrap();

    ids.into_iter()
        .map(|id| {
            let pty = ptys.get(&id);
            let status = SessionStatus {
                running: pty.is_some_and(|p| p.is_running()),
                exited: pty.is_some_and(|p| !p.is_running()),
                working: match (pty, sessions.get(&id)) {
                    (Some(p), Some(s)) if p.is_running() => activity::is_working(s, p, &busy),
                    _ => false,
                },
            };
            (id, status)
        })
        .collect()
}

#[tauri::command]
pub fn play_sound(name: String) -> CmdResult<()> {
    sound::play_ui_sound(&name)
}
