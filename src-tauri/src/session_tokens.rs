//! Tokens each Claude and Codex session has used, for the card header.
//!
//! Claude: the session's transcript, and the subagent transcripts beside it, are read incrementally with the
//! usage bar's reader, so a message counts once and cache reads are left out. Subagents count, because the
//! tokens are spent on the session's behalf.
//!
//! Codex: the rollout's last usage report that has data. Codex keeps a running total, so the last one is the
//! session's total. Its cached input is left out, as Claude's cache reads are.

use crate::usage::Meter;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// Bytes read from the end of a rollout. A usage report is a few hundred bytes, and the last one is near the end.
const CODEX_TAIL_BYTES: u64 = 256 * 1024;

/// Shared by every request, so each transcript is only read incrementally.
#[derive(Default)]
pub struct TokenState(pub Arc<Mutex<Tokens>>);

#[derive(Default)]
pub struct Tokens {
    /// One reader per Claude session, so messages streamed into several files count once.
    claude: HashMap<String, Meter>,
    claude_paths: HashMap<String, PathBuf>,
    codex_paths: HashMap<String, PathBuf>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub id: String,
    /// "claude" or "codex". Other kinds have no transcript, so they get no count.
    pub kind: String,
    pub agent_id: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Count {
    pub id: String,
    pub tokens: u64,
}

impl Tokens {
    /// The session's token total, or None when its transcript cannot be found yet.
    pub fn count(&mut self, home: &Path, request: &Request) -> Option<u64> {
        let agent_id = request.agent_id.as_deref()?;
        match request.kind.as_str() {
            "claude" => self.claude_count(&home.join(".claude/projects"), agent_id),
            "codex" => self.codex_count(&home.join(".codex/sessions"), agent_id),
            _ => None,
        }
    }

    fn claude_count(&mut self, projects: &Path, id: &str) -> Option<u64> {
        let transcript = match self.claude_paths.get(id) {
            Some(path) => path.clone(),
            None => {
                let path = find_claude(projects, id)?;
                self.claude_paths.insert(id.to_string(), path.clone());
                path
            }
        };
        let meter = self.claude.entry(id.to_string()).or_default();
        meter.read_new_lines(&transcript);
        for subagent in subagent_transcripts(&transcript) {
            meter.read_new_lines(&subagent);
        }
        Some(meter.billed_tokens())
    }

    fn codex_count(&mut self, sessions: &Path, id: &str) -> Option<u64> {
        let rollout = match self.codex_paths.get(id) {
            Some(path) => path.clone(),
            None => {
                let path = find_codex(sessions, 0, id)?;
                self.codex_paths.insert(id.to_string(), path.clone());
                path
            }
        };
        last_codex_total(&rollout)
    }
}

/// Claude keeps transcripts at projects/<encoded cwd>/<session id>.jsonl. Searching each folder avoids
/// re-deriving the encoded name, as the agent launcher does.
fn find_claude(projects: &Path, id: &str) -> Option<PathBuf> {
    let file = format!("{id}.jsonl");
    fs::read_dir(projects)
        .ok()?
        .flatten()
        .map(|entry| entry.path().join(&file))
        .find(|path| path.exists())
}

/// Subagent transcripts sit in projects/<folder>/<session id>/subagents/.
fn subagent_transcripts(transcript: &Path) -> Vec<PathBuf> {
    let dir = transcript.with_extension("").join("subagents");
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect()
}

/// Codex names each rollout after its session id, under sessions/<year>/<month>/<day>/.
fn find_codex(dir: &Path, depth: usize, id: &str) -> Option<PathBuf> {
    if depth > 3 {
        return None;
    }
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_codex(&path, depth + 1, id) {
                return Some(found);
            }
        } else if path.extension().is_some_and(|ext| ext == "jsonl")
            && path.file_name().is_some_and(|name| name.to_string_lossy().contains(id))
        {
            return Some(path);
        }
    }
    None
}

/// The newest usage report in a rollout that has data, as the session's total: input that was not cached,
/// plus output.
fn last_codex_total(path: &Path) -> Option<u64> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let from = len.saturating_sub(CODEX_TAIL_BYTES);
    file.seek(SeekFrom::Start(from)).ok()?;
    let mut tail = Vec::new();
    file.read_to_end(&mut tail).ok()?;
    let text = String::from_utf8_lossy(&tail);
    let mut lines: Vec<&str> = text.lines().collect();
    if from > 0 && !lines.is_empty() {
        lines.remove(0); // the read may have begun partway through a line
    }
    for line in lines.iter().rev() {
        if !line.contains("\"token_count\"") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
        let usage = &value["payload"]["info"]["total_token_usage"];
        if !usage.is_object() {
            continue; // a report without data, such as the first one of a session
        }
        let input = usage["input_tokens"].as_u64().unwrap_or(0);
        let cached = usage["cached_input_tokens"].as_u64().unwrap_or(0);
        let output = usage["output_tokens"].as_u64().unwrap_or(0);
        return Some(input.saturating_sub(cached) + output);
    }
    None
}

#[tauri::command]
pub async fn session_tokens(requests: Vec<Request>, state: tauri::State<'_, TokenState>) -> Result<Vec<Count>, String> {
    let tokens = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<Count>, String> {
        let home = dirs::home_dir().ok_or("Could not find the home folder")?;
        let mut tokens = tokens.lock().unwrap_or_else(|e| e.into_inner());
        Ok(requests
            .iter()
            .filter_map(|request| tokens.count(&home, request).map(|tokens| Count { id: request.id.clone(), tokens }))
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A scratch home folder that is removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("nesa-tokens-{}-{}", std::process::id(), name));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn append(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut file = fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        file.write_all(text.as_bytes()).unwrap();
    }

    fn assistant(id: &str, input: u64, cache_creation: u64, output: u64, cache_read: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"2026-10-09T10:00:00.000Z","message":{{"id":"{id}","usage":{{"input_tokens":{input},"cache_creation_input_tokens":{cache_creation},"output_tokens":{output},"cache_read_input_tokens":{cache_read}}}}}}}"#
        )
    }

    fn claude_request(id: &str) -> Request {
        Request { id: "session".into(), kind: "claude".into(), agent_id: Some(id.into()) }
    }

    #[test]
    fn a_claude_session_counts_each_message_once_and_leaves_cache_reads_out() {
        let scratch = Scratch::new("claude");
        let projects = scratch.0.join(".claude/projects");
        let transcript = projects.join("-work-api").join("s1.jsonl");
        append(&transcript, &format!("{}\n{}\n", assistant("m1", 10, 5, 1, 900), assistant("m1", 10, 5, 40, 900)));
        let mut tokens = Tokens::default();
        assert_eq!(tokens.count(&scratch.0, &claude_request("s1")), Some(10 + 5 + 40));

        append(&transcript, &format!("{}\n", assistant("m2", 3, 0, 7, 0)));
        assert_eq!(tokens.count(&scratch.0, &claude_request("s1")), Some(55 + 10), "new messages are added on the next read");
    }

    #[test]
    fn subagent_transcripts_beside_the_session_count_too() {
        let scratch = Scratch::new("subagents");
        let folder = scratch.0.join(".claude/projects/-work-api");
        append(&folder.join("s2.jsonl"), &format!("{}\n", assistant("main", 100, 0, 50, 0)));
        append(&folder.join("s2").join("subagents").join("agent-a.jsonl"), &format!("{}\n", assistant("sub", 20, 0, 5, 0)));
        let total = Tokens::default().count(&scratch.0, &claude_request("s2"));
        assert_eq!(total, Some(100 + 50 + 20 + 5));
    }

    #[test]
    fn a_session_without_a_transcript_has_no_count() {
        let scratch = Scratch::new("missing");
        assert_eq!(Tokens::default().count(&scratch.0, &claude_request("nope")), None);
        let browser = Request { id: "b".into(), kind: "browser".into(), agent_id: None };
        assert_eq!(Tokens::default().count(&scratch.0, &browser), None);
    }

    #[test]
    fn a_codex_session_uses_its_last_report_and_leaves_cached_input_out() {
        let scratch = Scratch::new("codex");
        let rollout = scratch.0.join(".codex/sessions/2026/10/09/rollout-2026-10-09T10-00-00-019f-abc123.jsonl");
        append(&rollout, r#"{"timestamp":"2026-10-09T10:00:00Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":null}}"#);
        append(&rollout, "\n");
        append(&rollout, r#"{"timestamp":"2026-10-09T10:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":200,"total_tokens":1200}}}}"#);
        append(&rollout, "\n");
        let request = Request { id: "codex".into(), kind: "codex".into(), agent_id: Some("abc123".into()) };
        assert_eq!(Tokens::default().count(&scratch.0, &request), Some(600 + 200));

        append(&rollout, r#"{"timestamp":"2026-10-09T10:05:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":5000,"cached_input_tokens":4000,"output_tokens":500}}}}"#);
        append(&rollout, "\n");
        let mut tokens = Tokens::default();
        assert_eq!(tokens.count(&scratch.0, &request), Some(1000 + 500), "the newest report is the total");
    }
}
