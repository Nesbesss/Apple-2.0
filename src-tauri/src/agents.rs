use crate::store::{Kind, Session};
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::SystemTime,
};

/// A command to run inside a PTY.
pub struct Launch {
    pub program: String,
    pub args: Vec<String>,
}

/// Runs the agent through the user's login shell, so PATH and shell-profile env match a normal
/// terminal. A GUI app does not inherit those.
pub fn launch_for(session: &Session) -> Launch {
    let shell = login_shell();
    let (binary, args) = match session.kind {
        Kind::Claude => claude_args(session),
        Kind::Codex => codex_args(session),
        // A plain terminal is the login shell itself, started the way Terminal.app starts one.
        Kind::Terminal => return Launch { program: shell, args: vec!["-i".into(), "-l".into()] },
        // A browser has no process. pty::spawn refuses it before this is ever run.
        Kind::Browser => return Launch { program: shell, args: Vec::new() },
    };

    let mut full = vec!["-i".into(), "-l".into(), "-c".into(), r#"exec "$0" "$@""#.into(), binary.into()];
    full.extend(args);
    Launch { program: shell, args: full }
}

pub(crate) fn login_shell() -> String {
    std::env::var("SHELL")
        .ok()
        .filter(|s| s.ends_with("zsh") || s.ends_with("bash"))
        .unwrap_or_else(|| "/bin/zsh".into())
}

fn claude_args(session: &Session) -> (&'static str, Vec<String>) {
    let id = session.agent_id.clone().unwrap_or_else(|| session.id.clone());
    if claude_transcript_exists(&id) {
        ("claude", vec!["--resume".into(), id])
    } else {
        // A brand-new session: pin its id so we know it before it ever writes a transcript.
        ("claude", vec!["--session-id".into(), id, "--name".into(), session.name.clone()])
    }
}

fn codex_args(session: &Session) -> (&'static str, Vec<String>) {
    match &session.agent_id {
        Some(id) => ("codex", vec!["resume".into(), id.clone()]),
        // Codex assigns its own id, so a new session is discovered from disk after launch.
        None => ("codex", vec![]),
    }
}

/// Claude stores transcripts at ~/.claude/projects/<encoded-cwd>/<session-id>.jsonl.
/// Searching every project folder avoids re-deriving the encoded directory name.
fn claude_transcript_exists(id: &str) -> bool {
    let Some(projects) = dirs::home_dir().map(|h| h.join(".claude/projects")) else {
        return false;
    };
    let file = format!("{id}.jsonl");
    fs::read_dir(projects)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| entry.path().join(&file).exists())
}

/// Finds the id of a Codex session started in `cwd` at or after `since`, skipping ids in `taken`.
pub fn discover_codex(cwd: &Path, since: SystemTime, taken: &[String]) -> Option<String> {
    let root = dirs::home_dir()?.join(".codex/sessions");
    let cwd = fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());

    for day in newest_days(&root, 2) {
        let Ok(entries) = fs::read_dir(&day) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "jsonl") {
                continue;
            }
            let created = entry.metadata().and_then(|m| m.created().or_else(|_| m.modified()));
            if !created.is_ok_and(|t| t >= since) {
                continue;
            }
            if let Some((id, file_cwd)) = read_session_meta(&path) {
                if file_cwd == cwd && !taken.contains(&id) {
                    return Some(id);
                }
            }
        }
    }
    None
}

/// Codex keeps rollouts under sessions/YYYY/MM/DD. Returns the newest `want` day folders.
fn newest_days(root: &Path, want: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for year in children_desc(root) {
        for month in children_desc(&year) {
            for day in children_desc(&month) {
                out.push(day);
                if out.len() == want {
                    return out;
                }
            }
        }
    }
    out
}

fn children_desc(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs.reverse();
    dirs
}

/// Reads the first line of a rollout: its session id and cwd. Subagent rollouts are skipped.
fn read_session_meta(path: &Path) -> Option<(String, PathBuf)> {
    let mut line = String::new();
    BufReader::new(fs::File::open(path).ok()?).read_line(&mut line).ok()?;
    let value: Value = serde_json::from_str(&line).ok()?;
    if value["type"] != "session_meta" || value["payload"]["source"].is_object() {
        return None;
    }
    let id = value["payload"]["id"].as_str()?.to_string();
    let cwd = PathBuf::from(value["payload"]["cwd"].as_str()?);
    Some((id, cwd))
}
