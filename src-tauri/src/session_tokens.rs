//! Tokens each Claude and Codex session has used, for the card header, and how much of its context window is left.
//!
//! Claude: the session's transcript, and the subagent transcripts beside it, are read incrementally with the
//! usage bar's reader, so a message counts once and cache reads are left out. Subagents count, because the
//! tokens are spent on the session's behalf.
//!
//! Codex: the rollout's last usage report that has data. Codex keeps a running total, so the last one is the
//! session's total. Its cached input is left out, as Claude's cache reads are. Subagents count too, for the same reason.
//!
//! The context figure (see `context`) and the subagent count (see `subagents`) come from the same transcripts.

use crate::{
    context::{self, Context},
    store::Kind,
    subagents::{self, Subagent, View},
    usage::{now_ms, window_start, Meter},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::UNIX_EPOCH,
};

/// Shared by every request, so each transcript is only read incrementally.
#[derive(Default)]
pub struct TokenState(pub Arc<Mutex<Tokens>>);

#[derive(Default)]
pub struct Tokens {
    /// One reader per Claude session, so messages streamed into several files count once.
    claude: HashMap<String, Meter>,
    claude_paths: HashMap<String, PathBuf>,
    codex_paths: HashMap<String, PathBuf>,
    /// Claude's last main-chain input, with the transcript's length and modified time when it was read.
    claude_inputs: HashMap<String, (u64, u64, Option<u64>)>,
    /// Codex's context, with the rollout's length and modified time when it was read.
    codex_contexts: HashMap<String, (u64, u64, Option<Context>)>,
    /// Subagent readers and rollout headers, shared by every session.
    subagents: subagents::Index,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub id: String,
    /// "claude" or "codex". Other kinds have no transcript, so they get no count.
    pub kind: String,
    pub agent_id: Option<String>,
    /// Whether to count the tokens, which reads the session's whole transcript. Without it a request gets its context and
    /// its subagent count only, which read far less.
    #[serde(default)]
    pub full: bool,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Count {
    pub id: String,
    pub tokens: u64,
    /// The session's weighted cost since the start of the block, in the units `Meter::window_units` uses.
    pub units: f64,
    /// How much of the context window is left. None until the transcript has a reading.
    pub context: Option<Context>,
    /// The subagents the session started, nested ones included.
    pub subagents: usize,
}

impl Tokens {
    /// The session's token total, or None when its transcript cannot be found yet.
    pub fn count(&mut self, home: &Path, request: &Request) -> Option<u64> {
        let agent_id = request.agent_id.as_deref()?;
        match request.kind.as_str() {
            "claude" => self.claude_count(home, agent_id),
            "codex" => self.codex_count(home, agent_id),
            _ => None,
        }
    }

    /// How full the session's context window is now. None when its transcript cannot be found or has no reading yet.
    pub fn context(&mut self, home: &Path, request: &Request) -> Option<Context> {
        let id = request.agent_id.as_deref()?;
        match kind_of(&request.kind)? {
            Kind::Claude => {
                let transcript = self.transcript(home, Kind::Claude, id)?;
                let (len, modified) = stamp(&transcript)?;
                let input = match self.claude_inputs.get(id) {
                    Some(&(l, m, input)) if l == len && m == modified => input,
                    _ => {
                        let input = context::claude_transcript_input(&transcript);
                        self.claude_inputs.insert(id.to_string(), (len, modified, input));
                        input
                    }
                };
                context::claude_context(input, modified, context::claude_report(&home.join(".claude"), id))
            }
            Kind::Codex => {
                let rollout = self.transcript(home, Kind::Codex, id)?;
                let (len, modified) = stamp(&rollout)?;
                match self.codex_contexts.get(id) {
                    Some(&(l, m, cached)) if l == len && m == modified => cached,
                    _ => {
                        let read = context::codex_context(&rollout);
                        self.codex_contexts.insert(id.to_string(), (len, modified, read));
                        read
                    }
                }
            }
            Kind::Terminal | Kind::Browser => None,
        }
    }

    /// How many subagents a session started, from the disk alone. Their transcripts are not read.
    pub fn subagent_count(&mut self, home: &Path, request: &Request) -> usize {
        let (Some(id), Some(kind)) = (request.agent_id.as_deref(), kind_of(&request.kind)) else { return 0 };
        let Some(transcript) = self.transcript(home, kind, id) else { return 0 };
        subagents::count(&mut self.subagents, kind, &transcript, &codex_root(home), id)
    }

    /// The subagents of a session, listed for the card.
    pub fn subagent_list(&mut self, home: &Path, kind: Kind, id: &str) -> Vec<Subagent> {
        let Some(transcript) = self.transcript(home, kind, id) else { return Vec::new() };
        subagents::list(&mut self.subagents, kind, &transcript, &codex_root(home), id)
    }

    /// One subagent of a session, for the viewer. None when the session has no subagent by that key.
    pub fn subagent_view(&mut self, home: &Path, kind: Kind, id: &str, key: &str) -> Option<View> {
        let transcript = self.transcript(home, kind, id)?;
        subagents::view(&mut self.subagents, kind, &transcript, &codex_root(home), id, key)
    }

    /// The session's own transcript, found once and then remembered. None when the agent has not written one yet.
    fn transcript(&mut self, home: &Path, kind: Kind, id: &str) -> Option<PathBuf> {
        match kind {
            Kind::Claude => cached(&mut self.claude_paths, id, || find_claude(&home.join(".claude/projects"), id)),
            Kind::Codex => cached(&mut self.codex_paths, id, || find_codex(&codex_root(home), 0, id)),
            Kind::Terminal | Kind::Browser => None,
        }
    }

    fn claude_count(&mut self, home: &Path, id: &str) -> Option<u64> {
        let transcript = self.transcript(home, Kind::Claude, id)?;
        let meter = self.claude.entry(id.to_string()).or_default();
        meter.read_new_lines(&transcript);
        for subagent in subagent_transcripts(&transcript) {
            meter.read_new_lines(&subagent);
        }
        Some(meter.billed_tokens())
    }

    fn codex_count(&mut self, home: &Path, id: &str) -> Option<u64> {
        let rollout = self.transcript(home, Kind::Codex, id)?;
        let own = last_codex_total(&rollout)?;
        let descendants = subagents::totals(&mut self.subagents, Kind::Codex, &rollout, &codex_root(home), id);
        Some(own + descendants.tokens)
    }
}

/// The agent a request names. Only Claude and Codex have transcripts.
fn kind_of(name: &str) -> Option<Kind> {
    match name {
        "claude" => Some(Kind::Claude),
        "codex" => Some(Kind::Codex),
        _ => None,
    }
}

fn codex_root(home: &Path) -> PathBuf {
    home.join(".codex/sessions")
}

/// The path remembered for `id`, found by `find` the first time it is asked for.
fn cached(paths: &mut HashMap<String, PathBuf>, id: &str, find: impl FnOnce() -> Option<PathBuf>) -> Option<PathBuf> {
    if let Some(path) = paths.get(id) {
        return Some(path.clone());
    }
    let path = find()?;
    paths.insert(id.to_string(), path.clone());
    Some(path)
}

/// A file's length and modified time in Unix millis. A cached reading is kept while both stay the same.
fn stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64;
    Some((meta.len(), modified))
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

/// The session's own total from the newest Codex report: input that was not cached, plus output.
fn last_codex_total(path: &Path) -> Option<u64> {
    context::codex_total(&context::codex_last_info(path)?["total_token_usage"])
}

#[tauri::command]
pub async fn session_tokens(
    requests: Vec<Request>,
    since_ms: Option<u64>,
    state: tauri::State<'_, TokenState>,
) -> Result<Vec<Count>, String> {
    let tokens = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<Count>, String> {
        let home = dirs::home_dir().ok_or("Could not find the home folder")?;
        let since = window_start(since_ms, now_ms());
        let mut tokens = tokens.lock().unwrap_or_else(|e| e.into_inner());
        Ok(requests
            .iter()
            .filter_map(|request| {
                let context = tokens.context(&home, request);
                let subagents = tokens.subagent_count(&home, request);
                if !request.full {
                    return Some(Count { id: request.id.clone(), tokens: 0, units: 0.0, context, subagents });
                }
                let total = tokens.count(&home, request)?;
                let units = request
                    .agent_id
                    .as_deref()
                    .and_then(|id| tokens.claude.get(id))
                    .map_or(0.0, |meter| meter.units_since(since));
                Some(Count { id: request.id.clone(), tokens: total, units, context, subagents })
            })
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
        Request { id: "session".into(), kind: "claude".into(), agent_id: Some(id.into()), full: true }
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
        let browser = Request { id: "b".into(), kind: "browser".into(), agent_id: None, full: true };
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
        let request = Request { id: "codex".into(), kind: "codex".into(), agent_id: Some("abc123".into()), full: true };
        assert_eq!(Tokens::default().count(&scratch.0, &request), Some(600 + 200));

        append(&rollout, r#"{"timestamp":"2026-10-09T10:05:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":5000,"cached_input_tokens":4000,"output_tokens":500}}}}"#);
        append(&rollout, "\n");
        let mut tokens = Tokens::default();
        assert_eq!(tokens.count(&scratch.0, &request), Some(1000 + 500), "the newest report is the total");
    }

    #[test]
    fn a_claude_session_reports_its_context_and_how_many_subagents_it_started() {
        let scratch = Scratch::new("context");
        let projects = scratch.0.join(".claude/projects");
        let transcript = projects.join("-work-api").join("s3.jsonl");
        let usage = r#"{"input_tokens":3,"cache_creation_input_tokens":1000,"cache_read_input_tokens":120000,"output_tokens":50}"#;
        let line = format!(r#"{{"type":"assistant","timestamp":"2026-10-09T10:00:00.000Z","message":{{"id":"m9","model":"claude-sonnet-5-5","usage":{usage}}}}}"#);
        append(&transcript, &format!("{line}\n"));
        append(&projects.join("-work-api").join("s3").join("subagents").join("agent-a1.jsonl"), &format!("{line}\n"));

        let mut tokens = Tokens::default();
        let request = claude_request("s3");
        // The input of the last call is the context: input, cache writes and cache reads, not the output.
        assert_eq!(tokens.context(&scratch.0, &request), Some(Context { used: 121_003, window: 200_000, exact: false }));
        assert_eq!(tokens.subagent_count(&scratch.0, &request), 1);
    }

    #[test]
    fn a_codex_session_counts_the_tokens_of_its_subagents() {
        let scratch = Scratch::new("codex-subagents");
        let root = scratch.0.join(".codex/sessions");
        let parent_id = "01a0fd3b-8f6e-7191-a8d1-15e905f89233";
        let child_id = "01a0fd3b-8f6e-7191-a8d1-15e905f89234";
        let parent = root.join("2026/10/09").join(format!("rollout-2026-10-09T09-00-00-{parent_id}.jsonl"));
        append(
            &parent,
            &format!(
                "{}\n{}\n",
                format!(r#"{{"timestamp":"2026-10-09T09:00:00Z","type":"session_meta","payload":{{"id":"{parent_id}","source":"cli"}}}}"#),
                r#"{"timestamp":"2026-10-09T09:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":200},"last_token_usage":{"input_tokens":700,"cached_input_tokens":400,"output_tokens":50},"model_context_window":258400}}}"#
            ),
        );
        let child = root.join("2026/10/09").join(format!("rollout-2026-10-09T09-30-00-{child_id}.jsonl"));
        append(
            &child,
            &format!(
                "{}\n{}\n",
                format!(
                    r#"{{"timestamp":"2026-10-09T09:30:00Z","type":"session_meta","payload":{{"id":"{child_id}","source":{{"subagent":{{"thread_spawn":{{"parent_thread_id":"{parent_id}","depth":1,"agent_nickname":"Dalton"}}}}}}}}}}"#
                ),
                r#"{"timestamp":"2026-10-09T09:31:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":500,"cached_input_tokens":100,"output_tokens":100}}}}"#
            ),
        );
        let request = Request { id: "codex".into(), kind: "codex".into(), agent_id: Some(parent_id.into()), full: true };
        let mut tokens = Tokens::default();
        assert_eq!(tokens.count(&scratch.0, &request), Some(800 + 500), "own 800, and the subagent's 500");
        assert_eq!(tokens.context(&scratch.0, &request), Some(Context { used: 700, window: 258_400, exact: true }));
        assert_eq!(tokens.subagent_count(&scratch.0, &request), 1);
    }

    #[test]
    fn a_light_request_counts_subagents_without_building_a_token_meter() {
        let scratch = Scratch::new("light");
        let projects = scratch.0.join(".claude/projects");
        append(&projects.join("-work-api").join("s4.jsonl"), &format!("{}\n", assistant("main", 100, 0, 50, 0)));
        append(&projects.join("-work-api").join("s4").join("subagents").join("agent-b1.jsonl"), &format!("{}\n", assistant("sub", 20, 0, 5, 0)));
        let light = Request { id: "session".into(), kind: "claude".into(), agent_id: Some("s4".into()), full: false };
        let mut tokens = Tokens::default();
        assert_eq!(tokens.subagent_count(&scratch.0, &light), 1);
        assert!(tokens.claude.is_empty(), "no transcript was read for its tokens");
    }

    /// Prints what a card would show for real sessions. Read only. Run with:
    /// `NESA_SESSIONS="claude:<id>,codex:<id>" cargo test real_sessions -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_sessions() {
        let home = dirs::home_dir().unwrap();
        let mut tokens = Tokens::default();
        for item in std::env::var("NESA_SESSIONS").unwrap().split(',') {
            let (kind, id) = item.split_once(':').unwrap();
            let request = Request { id: id.into(), kind: kind.into(), agent_id: Some(id.into()), full: true };
            println!("{kind} {id}: tokens {:?} context {:?}", tokens.count(&home, &request), tokens.context(&home, &request));
            println!("  subagents: {}", tokens.subagent_count(&home, &request));
            let agent = if kind == "claude" { Kind::Claude } else { Kind::Codex };
            for subagent in tokens.subagent_list(&home, agent, id) {
                println!("  {subagent:?}");
            }
        }
    }
}
