use crate::{
    activity, images, pty, sound, transcripts, web,
    store::{batch_names, default_name, random_names, Group, Kind, Session, Store},
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
pub fn delete_group(index: usize, app: AppHandle, state: State<'_, AppState>) -> CmdResult<Store> {
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
        web::close_browser(&app, &session.id);
    }
    Ok(snapshot)
}

/// Most sessions one request opens. A larger count is refused instead of starting a wall of processes.
pub const MAX_BATCH: usize = 8;

/// Starts `count` sessions of one kind in group `group_index`, named "name", "name 2", and so on. Each
/// session is saved as it starts, so when a later one fails the earlier ones stay, and the error says so.
#[tauri::command]
pub fn create_sessions(
    group_index: usize,
    kind: Kind,
    count: usize,
    name: String,
    cwd: String,
    url: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Vec<Session>> {
    if !(1..=MAX_BATCH).contains(&count) {
        return Err(format!("Open between 1 and {MAX_BATCH} sessions at a time"));
    }
    state.store.lock().unwrap().groups.get(group_index).ok_or("That group no longer exists")?;

    let typed = name.trim().to_string();
    let (cwd, base, url) = match kind {
        Kind::Browser => {
            // A browser has no folder. Its address is optional, and without a typed name the host names it.
            let url = match url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
                Some(text) => Some(web::normalize_url(text)?),
                None => None,
            };
            let base = if typed.is_empty() {
                url.as_ref().and_then(|u| u.host_str()).unwrap_or("Browser").to_string()
            } else {
                typed.clone()
            };
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
            (home, base, url.map(|u| u.to_string()))
        }
        _ => {
            let path = expand_home(cwd.trim());
            let cwd = fs::canonicalize(&path).map_err(|_| format!("No such folder: {}", path.display()))?;
            if !cwd.is_dir() {
                return Err(format!("Not a folder: {}", cwd.display()));
            }
            // The folder name says far more than "Claude 3", and it is what people look for.
            let folder = cwd
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| kind.label().to_string());
            let base = if typed.is_empty() { folder } else { typed.clone() };
            (cwd, base, None)
        }
    };

    // One session keeps a typed name exactly. Several are numbered from the base, around the names in use.
    // Claude and Codex sessions opened without a name get a friendly one, which is also what Claude shows
    // for the session, since the name is passed to it on launch.
    let names = if count == 1 && !typed.is_empty() {
        vec![typed.clone()]
    } else {
        let store = state.store.lock().unwrap();
        let group = store.groups.get(group_index).ok_or("That group no longer exists")?;
        let taken: Vec<&str> = group.sessions.iter().map(|s| s.name.as_str()).collect();
        if typed.is_empty() && matches!(kind, Kind::Claude | Kind::Codex) {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos() as usize);
            random_names(&taken, count, seed)
        } else {
            batch_names(&base, &taken, count)
        }
    };

    let mut created = Vec::with_capacity(count);
    for (started, name) in names.into_iter().enumerate() {
        let id = Uuid::new_v4().to_string();
        // Claude takes a caller-chosen id, so the same id is used to launch and to resume.
        let agent_id = (kind == Kind::Claude).then(|| id.clone());
        let session = Session { id, kind, name, cwd: cwd.clone(), agent_id, url: url.clone() };
        start_session(&app, &state, group_index, session.clone()).map_err(|e| {
            if started == 0 { e } else { format!("Started {started} of {count}. {e}") }
        })?;
        created.push(session);
    }
    Ok(created)
}

/// Starts a session's agent and files it under group `group_index`. New and resumed sessions both go through
/// here. A browser has no process, so only its place in the store is recorded.
fn start_session(app: &AppHandle, state: &AppState, group_index: usize, session: Session) -> CmdResult<Session> {
    if session.kind.has_pty() {
        let mut ptys = state.ptys.lock().unwrap();
        let pty = pty::spawn(app, &session)?;
        ptys.insert(session.id.clone(), pty);
    }

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
    let session = Session { id: Uuid::new_v4().to_string(), kind, name, cwd: transcript.cwd, agent_id: Some(agent_id), url: None };
    start_session(&app, &state, group_index, session)
}

#[tauri::command]
pub fn remove_session(id: String, app: AppHandle, state: State<'_, AppState>) -> CmdResult<Store> {
    if let Some(mut pty) = state.ptys.lock().unwrap().remove(&id) {
        pty.stop();
    }
    web::close_browser(&app, &id);
    let mut store = state.store.lock().unwrap();
    for group in &mut store.groups {
        group.sessions.retain(|s| s.id != id);
    }
    save(&store)?;
    Ok(store.clone())
}

/// Shows the session's folder in Finder. Only folders Nesa already knows about can be shown.
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

/// Puts a session into group `group`, before the session at `index`. Used by drag and drop in the sidebar.
#[tauri::command]
pub fn place_session(id: String, group: usize, index: usize, state: State<'_, AppState>) -> CmdResult<Store> {
    edit_store(&state, |store| store.place_session(&id, group, index))
}

/// Moves group `from` to sit before the group at `index`. Used by drag and drop in the sidebar.
#[tauri::command]
pub fn move_group(from: usize, index: usize, state: State<'_, AppState>) -> CmdResult<Store> {
    edit_store(&state, |store| store.move_group(from, index))
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
    if !session.kind.has_pty() {
        return Ok(());
    }
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
    if !session.kind.has_pty() {
        return Ok(());
    }
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

/// Saves the sound a finished agent plays. The UI sends it on launch and whenever it changes.
#[tauri::command]
pub fn set_done_ping(name: String) -> CmdResult<()> {
    sound::set_done_ping(&name)
}

/// Plays one done ping so the user can hear it in settings.
#[tauri::command]
pub fn preview_done_ping(name: String) -> CmdResult<()> {
    sound::preview_done_ping(&name)
}

/// Most paths one request checks. A screen of links is far fewer.
const MAX_IMAGE_CANDIDATES: usize = 64;

/// Turns paths a terminal shows into the image files they name, or None for each that is not one. Each
/// path is looked up against the session's folder. This runs off the UI thread, since it can walk folders.
#[tauri::command]
pub async fn resolve_images(cwd: String, candidates: Vec<String>) -> CmdResult<Vec<Option<String>>> {
    if candidates.len() > MAX_IMAGE_CANDIDATES {
        return Err("Too many paths at once".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let cwd = PathBuf::from(cwd);
        candidates
            .iter()
            .map(|text| images::resolve(&cwd, text).map(|path| path.display().to_string()))
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// The image at `path` as a data URL, for the viewer. Only image files are read.
#[tauri::command]
pub async fn read_image(path: String) -> CmdResult<String> {
    tauri::async_runtime::spawn_blocking(move || images::data_url(std::path::Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
}

/// Writes a file dropped on the window into a cache folder and returns its path. The agent reads the
/// file from there, so a dropped image becomes an attachment, the same as a pasted one.
#[tauri::command]
pub fn save_dropped_file(name: String, data_base64: String) -> CmdResult<String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let bytes = STANDARD.decode(data_base64).map_err(|e| format!("Could not read the file: {e}"))?;
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
        .collect();
    let cleaned = if cleaned.is_empty() { "file".to_string() } else { cleaned };
    let dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("SessionDeck")
        .join("drops");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let path = dir.join(format!("{}-{cleaned}", Uuid::new_v4()));
    fs::write(&path, bytes).map_err(|e| format!("Could not save {cleaned}: {e}"))?;
    Ok(path.display().to_string())
}
