use crate::{
    agents,
    pty::Pty,
    sound,
    store::{Kind, Session},
    AppState,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs, thread,
    time::{Duration, SystemTime},
};
use tauri::{AppHandle, Emitter, Manager};

const TICK: Duration = Duration::from_millis(500);
/// Codex has no status file. Its working text is redrawn while busy, so it must be seen this recently.
const CODEX_BUSY_WINDOW: Duration = Duration::from_secs(2);
/// Codex ids are discovered every few ticks until found.
const CODEX_DISCOVERY_EVERY: u64 = 4;

#[derive(Clone, Serialize)]
pub struct ActivityEvent {
    pub id: String,
    pub working: bool,
}

/// Session ids that Claude reports as busy, from the status files in ~/.claude/sessions.
pub fn claude_busy_ids() -> HashSet<String> {
    let Some(dir) = dirs::home_dir().map(|h| h.join(".claude/sessions")) else {
        return HashSet::new();
    };
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| serde_json::from_slice::<Value>(&fs::read(path).ok()?).ok())
        .filter(|status| status["status"] == "busy")
        .filter_map(|status| status["sessionId"].as_str().map(str::to_owned))
        .collect()
}

/// Whether a running session is busy right now.
pub fn is_working(session: &Session, pty: &Pty, busy: &HashSet<String>) -> bool {
    match session.kind {
        Kind::Claude => session.agent_id.as_ref().is_some_and(|id| busy.contains(id)),
        Kind::Codex => pty.busy_text_within(CODEX_BUSY_WINDOW),
    }
}

/// How many sessions are busy right now. Used to decide whether quitting needs a confirmation.
pub fn working_count(state: &AppState) -> usize {
    let sessions: Vec<Session> = state
        .store
        .lock()
        .unwrap()
        .groups
        .iter()
        .flat_map(|g| g.sessions.clone())
        .collect();
    let busy = claude_busy_ids();
    let ptys = state.ptys.lock().unwrap();
    sessions
        .iter()
        .filter(|s| ptys.get(&s.id).is_some_and(|p| p.is_running() && is_working(s, p, &busy)))
        .count()
}

/// Background loop: emits `activity` on every change, plays the done sound on working -> idle,
/// and discovers Codex session ids once they appear on disk.
pub fn spawn_watcher(app: AppHandle) {
    thread::spawn(move || {
        let mut previous: HashMap<String, bool> = HashMap::new();
        let mut tick: u64 = 0;
        loop {
            thread::sleep(TICK);
            tick += 1;
            let state = app.state::<AppState>();
            let sessions: Vec<Session> = state
                .store
                .lock()
                .unwrap()
                .groups
                .iter()
                .flat_map(|g| g.sessions.clone())
                .collect();
            let busy = claude_busy_ids();

            let mut changes = Vec::new();
            let mut finished = false;
            let mut discovery = Vec::new();
            let mut seen = HashSet::new();
            {
                let ptys = state.ptys.lock().unwrap();
                for session in &sessions {
                    let Some(pty) = ptys.get(&session.id) else { continue };
                    if !pty.is_running() {
                        continue;
                    }
                    seen.insert(session.id.clone());
                    let working = is_working(session, pty, &busy);
                    let before = previous.insert(session.id.clone(), working);
                    if before != Some(working) {
                        changes.push(ActivityEvent { id: session.id.clone(), working });
                    }
                    if before == Some(true) && !working {
                        finished = true;
                    }
                    if session.kind == Kind::Codex && session.agent_id.is_none() && tick % CODEX_DISCOVERY_EVERY == 0 {
                        let since = pty.started.checked_sub(Duration::from_secs(2)).unwrap_or(pty.started);
                        discovery.push((session.id.clone(), session.cwd.clone(), since));
                    }
                }
            }
            // A busy session whose process ended must not stay "working" in the UI.
            let gone: Vec<String> = previous.keys().filter(|id| !seen.contains(*id)).cloned().collect();
            for id in gone {
                if previous.remove(&id) == Some(true) {
                    changes.push(ActivityEvent { id, working: false });
                }
            }

            for change in changes {
                let _ = app.emit("activity", change);
            }
            if finished {
                sound::agent_done();
            }
            if !discovery.is_empty() {
                resolve_codex_ids(&app, &sessions, discovery);
            }
        }
    });
}

/// Matches each pending Codex session to its rollout file, then saves and announces the ids.
fn resolve_codex_ids(app: &AppHandle, sessions: &[Session], pending: Vec<(String, std::path::PathBuf, SystemTime)>) {
    let mut taken: Vec<String> = sessions.iter().filter_map(|s| s.agent_id.clone()).collect();
    let mut found = Vec::new();
    for (id, cwd, since) in pending {
        if let Some(agent_id) = agents::discover_codex(&cwd, since, &taken) {
            taken.push(agent_id.clone());
            found.push((id, agent_id));
        }
    }
    if found.is_empty() {
        return;
    }

    let state = app.state::<AppState>();
    let mut store = state.store.lock().unwrap();
    for group in &mut store.groups {
        for session in &mut group.sessions {
            if let Some((_, agent_id)) = found.iter().find(|(id, _)| *id == session.id) {
                session.agent_id = Some(agent_id.clone());
            }
        }
    }
    let _ = store.save();
    let snapshot = store.clone();
    drop(store);
    let _ = app.emit("store-changed", snapshot);
}
