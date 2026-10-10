//! Subagents: the agents a Claude or Codex session started, and what each one has done. The session's card lists them
//! as a subdirectory, and one opens at a time.
//!
//! Claude keeps each subagent beside the session's transcript: <folder>/<session id>/subagents/agent-<key>.jsonl, with
//! agent-<key>.meta.json naming its task, type and model. Codex writes each subagent as a rollout of its own, and the
//! first line of that rollout names the session that started it. Nested subagents are found by walking down from the
//! session.
//!
//! Each transcript is read by a Reader that is kept between calls, so a poll reads only what was appended. Text leaves
//! here unchanged, and the frontend shows it as text, never as HTML.

use crate::{
    context::codex_total,
    session_tokens::TokenState,
    store::Kind,
    transcripts::is_valid_id,
    usage::{parse_iso_ms, read_appended, Cursor, Meter},
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

/// Entries kept for the viewer. The task, the first user message, is kept whatever the window holds.
const ENTRY_WINDOW: usize = 300;
/// Longest text kept for one entry. A longer one, such as a big tool result, is cut with an ellipsis.
const TEXT_CHARS: usize = 4000;

/// Readers and Codex rollout headers, kept between calls.
#[derive(Default)]
pub struct Index {
    readers: HashMap<PathBuf, Reader>,
    /// Codex rollout headers by path. None marks a rollout that no session started.
    heads: HashMap<PathBuf, Option<Head>>,
}

/// One subagent, as the card lists it.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Subagent {
    /// Names the subagent within its session. Claude: the agent id in its file name. Codex: its session id.
    pub key: String,
    /// What the subagent was asked to do, in a few words. Claude: the description it was started with.
    pub label: String,
    /// The kind of agent. Claude: its agent type. Codex: its role or path. Empty when neither is known.
    pub role: String,
    pub model: Option<String>,
    /// How deep it sits under the session: 1 for one the session started.
    pub depth: u32,
    /// The subagent that started this one, or None when the session started it.
    pub parent: Option<String>,
    /// False once the transcript shows the agent has ended its turn. Whether it is still running is for the caller to say.
    pub busy: bool,
    pub tokens: u64,
    pub started_ms: u64,
    pub updated_ms: u64,
}

/// One message, call or result in a subagent's transcript.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// "user", "assistant", "tool" (a call), or "result" (what a call returned).
    pub role: &'static str,
    /// The tool's name, for a call. Empty otherwise.
    pub name: String,
    pub text: String,
    pub at_ms: u64,
}

/// A subagent with the entries the viewer shows.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub subagent: Subagent,
    pub entries: Vec<Entry>,
    /// Every entry the transcript holds. `entries` can be fewer when the transcript is long.
    pub total: usize,
}

/// The subagents a session started, counted, with the tokens they have used.
#[derive(Default, Debug, PartialEq)]
pub struct Totals {
    pub count: usize,
    pub tokens: u64,
}

/// Where a subagent's transcript is, and who started it. Read from the disk, before the transcript itself.
#[derive(Clone, Debug)]
struct Meta {
    key: String,
    label: String,
    role: String,
    model: Option<String>,
    depth: u32,
    parent: Option<String>,
    path: PathBuf,
}

/// The first line of a Codex rollout that a session started.
#[derive(Clone, Debug)]
struct Head {
    id: String,
    parent: Option<String>,
    depth: u32,
    nickname: Option<String>,
    role: Option<String>,
    agent_path: Option<String>,
}

impl Index {
    /// The header of a Codex rollout, read once. None when no session started the rollout.
    fn head(&mut self, path: &Path) -> Option<Head> {
        if !self.heads.contains_key(path) {
            let head = read_head(path);
            self.heads.insert(path.to_path_buf(), head);
        }
        self.heads.get(path).cloned().flatten()
    }
}

/// The subagents a session started, and the ones they started, in the order they began. `transcript` is the session's
/// own transcript, and `codex_root` the folder that holds every Codex rollout.
pub fn list(index: &mut Index, kind: Kind, transcript: &Path, codex_root: &Path, session_id: &str) -> Vec<Subagent> {
    let metas = discover(index, kind, transcript, codex_root, session_id);
    let mut out: Vec<Subagent> = metas.iter().map(|meta| refresh(index, kind, meta).summary(meta, kind)).collect();
    out.sort_by_key(|subagent| subagent.started_ms);
    out
}

/// The subagents under a session, counted, with their tokens.
pub fn totals(index: &mut Index, kind: Kind, transcript: &Path, codex_root: &Path, session_id: &str) -> Totals {
    let metas = discover(index, kind, transcript, codex_root, session_id);
    let mut totals = Totals::default();
    for meta in &metas {
        totals.count += 1;
        totals.tokens += refresh(index, kind, meta).tokens(kind);
    }
    totals
}

/// One subagent with the entries the viewer shows. None when the session has no subagent by that key.
pub fn view(index: &mut Index, kind: Kind, transcript: &Path, codex_root: &Path, session_id: &str, key: &str) -> Option<View> {
    if !is_valid_key(key) {
        return None;
    }
    let meta = discover(index, kind, transcript, codex_root, session_id).into_iter().find(|meta| meta.key == key)?;
    let reader = refresh(index, kind, &meta);
    Some(View { subagent: reader.summary(&meta, kind), entries: reader.entries(), total: reader.total })
}

/// Brings a subagent's reader up to date with its transcript, and returns it.
fn refresh<'a>(index: &'a mut Index, kind: Kind, meta: &Meta) -> &'a Reader {
    let reader = index.readers.entry(meta.path.clone()).or_insert_with(Reader::new);
    reader.refresh(kind, &meta.path);
    reader
}

fn discover(index: &mut Index, kind: Kind, transcript: &Path, codex_root: &Path, session_id: &str) -> Vec<Meta> {
    match kind {
        Kind::Claude => claude_metas(transcript),
        Kind::Codex => codex_metas(index, codex_root, transcript, session_id),
        Kind::Terminal | Kind::Browser => Vec::new(),
    }
}

/// Claude's subagents: every agent-<key>.jsonl beside the session, with the task and type its meta file names.
fn claude_metas(transcript: &Path) -> Vec<Meta> {
    let dir = transcript.with_extension("").join("subagents");
    let Ok(entries) = fs::read_dir(&dir) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(key) = name.to_str().and_then(|n| n.strip_prefix("agent-")).and_then(|n| n.strip_suffix(".jsonl")) else {
            continue;
        };
        if !is_valid_key(key) {
            continue;
        }
        let meta: Value = fs::read(dir.join(format!("agent-{key}.meta.json")))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(Value::Null);
        out.push(Meta {
            key: key.to_string(),
            label: meta["description"].as_str().filter(|d| !d.is_empty()).unwrap_or("Subagent").to_string(),
            role: meta["agentType"].as_str().unwrap_or_default().to_string(),
            model: meta["model"].as_str().map(str::to_string),
            depth: meta["spawnDepth"].as_u64().unwrap_or(1) as u32,
            parent: None,
            path: entry.path(),
        });
    }
    out
}

/// Codex's subagents under a session: every rollout from the session's day on whose header names a parent already in
/// the tree. Subagents of subagents are included, so the walk repeats until nothing new joins.
fn codex_metas(index: &mut Index, root: &Path, rollout: &Path, session_id: &str) -> Vec<Meta> {
    let Some(from) = day_of(root, rollout) else { return Vec::new() };
    let mut found = Vec::new();
    for path in rollouts_from(root, &from) {
        if let Some(head) = index.head(&path) {
            found.push((head, path));
        }
    }
    let mut known: HashSet<String> = HashSet::from([session_id.to_string()]);
    let mut out = Vec::new();
    loop {
        let before = out.len();
        for (head, path) in &found {
            let Some(parent) = head.parent.as_ref().filter(|parent| known.contains(*parent)) else { continue };
            if known.contains(&head.id) {
                continue;
            }
            known.insert(head.id.clone());
            out.push(Meta {
                key: head.id.clone(),
                label: head.nickname.clone().unwrap_or_else(|| "Subagent".into()),
                role: head.role.clone().or_else(|| head.agent_path.clone()).unwrap_or_default(),
                model: None,
                depth: head.depth,
                parent: (parent.as_str() != session_id).then(|| parent.clone()),
                path: path.clone(),
            });
        }
        if out.len() == before {
            return out;
        }
    }
}

/// The day folder a rollout sits in, as "YYYY/MM/DD" relative to the sessions folder.
fn day_of(root: &Path, rollout: &Path) -> Option<String> {
    let parts: Vec<String> = rollout
        .strip_prefix(root)
        .ok()?
        .components()
        .take(3)
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    (parts.len() == 3).then(|| parts.join("/"))
}

/// Every rollout in a day folder on or after `from` ("YYYY/MM/DD"). The folders are named by date, so comparing the
/// names orders them.
fn rollouts_from(root: &Path, from: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for year in dirs_in(root) {
        for month in dirs_in(&year) {
            for day in dirs_in(&month) {
                let key = format!("{}/{}/{}", name_of(&year), name_of(&month), name_of(&day));
                if key.as_str() < from {
                    continue;
                }
                out.extend(files_in(&day).into_iter().filter(|file| {
                    file.extension().is_some_and(|ext| ext == "jsonl")
                        && file.file_name().is_some_and(|name| name.to_string_lossy().starts_with("rollout-"))
                }));
            }
        }
    }
    out
}

fn dirs_in(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    out.sort();
    out
}

fn files_in(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    out.sort();
    out
}

fn name_of(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

/// A Codex rollout's first line, if a session started it: the session it came from, its depth and its nickname.
fn read_head(path: &Path) -> Option<Head> {
    let mut line = String::new();
    BufReader::new(File::open(path).ok()?).read_line(&mut line).ok()?;
    let value: Value = serde_json::from_str(&line).ok()?;
    let payload = &value["payload"];
    if value["type"] != "session_meta" || !payload["source"]["subagent"].is_object() {
        return None;
    }
    let spawn = &payload["source"]["subagent"]["thread_spawn"];
    let id = payload["id"].as_str().filter(|id| is_valid_key(id))?.to_string();
    let text = |v: &Value| v.as_str().map(str::to_string);
    Some(Head {
        id,
        parent: text(&spawn["parent_thread_id"]).or_else(|| text(&payload["parent_thread_id"])),
        depth: spawn["depth"].as_u64().unwrap_or(1) as u32,
        nickname: text(&spawn["agent_nickname"]).or_else(|| text(&payload["agent_nickname"])),
        role: text(&spawn["agent_role"]),
        agent_path: text(&spawn["agent_path"]),
    })
}

/// Subagent keys are short ids of letters, digits and dashes, which Claude and Codex both use. Anything else never names a
/// file, so a key from the frontend cannot reach outside the subagent folders.
fn is_valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 96 && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Subagents belong to Claude and Codex sessions only, and their session ids are UUIDs. Anything else is refused before
/// a transcript path is looked up.
fn check_session(kind: Kind, session_id: &str) -> Result<(), String> {
    if !matches!(kind, Kind::Claude | Kind::Codex) || !is_valid_id(session_id) {
        return Err("That session has no subagents".into());
    }
    Ok(())
}

/// The subagents a session started, for the card's subdirectory. Reads transcripts, so it runs off the UI thread.
#[tauri::command]
pub async fn list_subagents(kind: Kind, agent_id: String, state: tauri::State<'_, TokenState>) -> Result<Vec<Subagent>, String> {
    check_session(kind, &agent_id)?;
    let tokens = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<Subagent>, String> {
        let home = dirs::home_dir().ok_or("Could not find the home folder")?;
        let mut tokens = tokens.lock().unwrap_or_else(|e| e.into_inner());
        Ok(tokens.subagent_list(&home, kind, &agent_id))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// One subagent, for the viewer. The frontend names it by the key the listing gave, never by a path.
#[tauri::command]
pub async fn subagent_view(kind: Kind, agent_id: String, key: String, state: tauri::State<'_, TokenState>) -> Result<View, String> {
    check_session(kind, &agent_id)?;
    let tokens = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<View, String> {
        let home = dirs::home_dir().ok_or("Could not find the home folder")?;
        let mut tokens = tokens.lock().unwrap_or_else(|e| e.into_inner());
        tokens.subagent_view(&home, kind, &agent_id, &key).ok_or_else(|| "That subagent is no longer on disk".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A subagent's transcript, read as it grows. It keeps the tokens the agent has used, whether its last turn has ended,
/// and the newest entries.
struct Reader {
    cursor: Cursor,
    /// Claude: the tokens its messages used, counted the way the usage bar counts them.
    meter: Meter,
    /// Codex: the total of its newest token report.
    codex_total: Option<u64>,
    entries: VecDeque<Entry>,
    /// The first user message and its place in the transcript. Kept after the window moves past it.
    first: Option<(usize, Entry)>,
    total: usize,
    busy: bool,
    started_ms: u64,
    updated_ms: u64,
}

impl Reader {
    fn new() -> Self {
        Reader {
            cursor: Cursor::default(),
            meter: Meter::default(),
            codex_total: None,
            entries: VecDeque::new(),
            first: None,
            total: 0,
            busy: true,
            started_ms: 0,
            updated_ms: 0,
        }
    }

    fn refresh(&mut self, kind: Kind, path: &Path) {
        for line in read_appended(path, &mut self.cursor) {
            match kind {
                Kind::Claude => self.claude_line(&line),
                Kind::Codex => self.codex_line(&line),
                Kind::Terminal | Kind::Browser => {}
            }
        }
    }

    fn tokens(&self, kind: Kind) -> u64 {
        match kind {
            Kind::Claude => self.meter.billed_tokens(),
            Kind::Codex => self.codex_total.unwrap_or(0),
            Kind::Terminal | Kind::Browser => 0,
        }
    }

    fn summary(&self, meta: &Meta, kind: Kind) -> Subagent {
        Subagent {
            key: meta.key.clone(),
            label: meta.label.clone(),
            role: meta.role.clone(),
            model: meta.model.clone(),
            depth: meta.depth,
            parent: meta.parent.clone(),
            busy: self.busy,
            tokens: self.tokens(kind),
            started_ms: self.started_ms,
            updated_ms: self.updated_ms,
        }
    }

    /// The entries to show: the task first when the window has moved past it, then the newest entries.
    fn entries(&self) -> Vec<Entry> {
        let dropped = self.total - self.entries.len();
        let mut out = Vec::with_capacity(self.entries.len() + 1);
        if let Some((index, first)) = &self.first {
            if *index < dropped {
                out.push(first.clone());
            }
        }
        out.extend(self.entries.iter().cloned());
        out
    }

    fn push(&mut self, role: &'static str, name: &str, text: &str, at_ms: u64) {
        if text.trim().is_empty() {
            return;
        }
        let entry = Entry { role, name: name.to_string(), text: clip(text, TEXT_CHARS), at_ms };
        if self.first.is_none() && role == "user" {
            self.first = Some((self.total, entry.clone()));
        }
        self.total += 1;
        self.entries.push_back(entry);
        if self.entries.len() > ENTRY_WINDOW {
            self.entries.pop_front();
        }
    }

    /// Records a line's time. The first line starts the subagent, and the newest line updates it.
    fn note_time(&mut self, value: &Value) -> u64 {
        let at = value["timestamp"].as_str().and_then(parse_iso_ms).unwrap_or(0);
        if at > 0 {
            if self.started_ms == 0 {
                self.started_ms = at;
            }
            self.updated_ms = self.updated_ms.max(at);
        }
        at
    }

    fn claude_line(&mut self, line: &str) {
        self.meter.apply_line(line);
        let Ok(value) = serde_json::from_str::<Value>(line) else { return };
        let at = self.note_time(&value);
        match value["type"].as_str() {
            Some("user") => {
                // A user line is the task or a tool's result. Either way the agent is working on it.
                self.busy = true;
                match &value["message"]["content"] {
                    Value::String(text) => self.push("user", "", text, at),
                    Value::Array(blocks) => {
                        for block in blocks {
                            match block["type"].as_str() {
                                Some("text") => self.push("user", "", block["text"].as_str().unwrap_or_default(), at),
                                Some("tool_result") => self.push("result", "", &text_of(&block["content"]), at),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            Some("assistant") => {
                let message = &value["message"];
                for block in message["content"].as_array().into_iter().flatten() {
                    match block["type"].as_str() {
                        Some("text") => self.push("assistant", "", block["text"].as_str().unwrap_or_default(), at),
                        Some("tool_use") | Some("server_tool_use") => {
                            let name = block["name"].as_str().unwrap_or("tool");
                            self.push("tool", name, &summarize(&block["input"]), at);
                        }
                        _ => {}
                    }
                }
                // Thinking and the advisor's blocks are left out. A reply that ends its turn is the last one.
                self.busy = message["stop_reason"] != "end_turn";
            }
            _ => {}
        }
    }

    fn codex_line(&mut self, line: &str) {
        if line.contains("\"token_count\"") {
            if let Some(total) = serde_json::from_str::<Value>(line).ok().and_then(|v| codex_total(&v["payload"]["info"]["total_token_usage"])) {
                self.codex_total = Some(total);
            }
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else { return };
        let at = self.note_time(&value);
        let payload = &value["payload"];
        match (value["type"].as_str(), payload["type"].as_str()) {
            (Some("event_msg"), Some("task_started")) => self.busy = true,
            (Some("event_msg"), Some("task_complete" | "turn_aborted")) => self.busy = false,
            (Some("response_item"), Some("message")) => match payload["role"].as_str() {
                Some("user") => self.push("user", "", &text_of(&payload["content"]), at),
                Some("assistant") => self.push("assistant", "", &text_of(&payload["content"]), at),
                _ => {}
            },
            (Some("response_item"), Some("agent_message")) => self.push("assistant", "", &text_of(&payload["content"]), at),
            (Some("response_item"), Some("function_call" | "custom_tool_call")) => {
                // A function call carries its input as `arguments`, and a custom tool call as `input`.
                let input = if payload["arguments"].is_null() { &payload["input"] } else { &payload["arguments"] };
                self.push("tool", payload["name"].as_str().unwrap_or("tool"), &summarize(input), at);
            }
            (Some("response_item"), Some("function_call_output" | "custom_tool_call_output")) => {
                self.push("result", "", &text_of(&payload["output"]), at);
            }
            _ => {}
        }
    }
}

/// The text of a message, a tool's output or a content list: a string as it is, or the text blocks of a list joined.
fn text_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items.iter().filter_map(|item| item["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

/// A tool's input as text: a string as it is, anything else as compact JSON.
fn summarize(input: &Value) -> String {
    match input {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// The first `limit` characters of `text`, with an ellipsis when something was cut.
fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out: String = text.chars().take(limit).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use uuid::Uuid;

    /// A scratch folder that is removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("nesa-subagents-{}-{}", Uuid::new_v4(), name));
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

    const SESSION: &str = "2e23b561-73b2-4975-8149-fc9fd89ddf39";
    const PARENT: &str = "01a0fd3b-8f6e-7191-a8d1-15e905f89233";
    const CHILD: &str = "01a0fd3b-8f6e-7191-a8d1-15e905f89234";
    const GRANDCHILD: &str = "01a0fd3b-8f6e-7191-a8d1-15e905f89235";
    const OTHER: &str = "01a0fd3b-8f6e-7191-a8d1-15e905f89236";

    fn claude_user(text: &str, ts: &str) -> String {
        format!(r#"{{"type":"user","isSidechain":true,"timestamp":"{ts}","message":{{"role":"user","content":"{text}"}}}}"#)
    }

    fn claude_reply(text: &str, stop: Option<&str>, ts: &str) -> String {
        let stop = stop.map_or("null".to_string(), |s| format!("\"{s}\""));
        format!(
            r#"{{"type":"assistant","isSidechain":true,"timestamp":"{ts}","message":{{"id":"m-{ts}","model":"claude-sonnet-5-5","stop_reason":{stop},"content":[{{"type":"text","text":"{text}"}}],"usage":{{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":500,"output_tokens":5}}}}}}"#
        )
    }

    fn codex_head(id: &str, parent: Option<&str>, depth: u32, nickname: &str, ts: &str) -> String {
        let parent = parent.map_or("null".to_string(), |p| format!("\"{p}\""));
        format!(
            r#"{{"timestamp":"{ts}","type":"session_meta","payload":{{"id":"{id}","cwd":"/work/app","source":{{"subagent":{{"thread_spawn":{{"parent_thread_id":{parent},"depth":{depth},"agent_nickname":"{nickname}","agent_role":null,"agent_path":"/root/router"}}}}}}}}}}"#
        )
    }

    fn codex_event(kind: &str, ts: &str) -> String {
        format!(r#"{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"{kind}"}}}}"#)
    }

    fn codex_prompt(text: &str, ts: &str) -> String {
        format!(r#"{{"timestamp":"{ts}","type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"{text}"}}]}}}}"#)
    }

    fn codex_tokens(ts: &str, input: u64, cached: u64, output: u64) -> String {
        format!(
            r#"{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":{input},"cached_input_tokens":{cached},"output_tokens":{output}}}}}}}}}"#
        )
    }

    #[test]
    fn claude_subagents_are_listed_with_their_task_tokens_and_whether_they_are_busy() {
        let scratch = Scratch::new("claude-list");
        let transcript = scratch.0.join("projects/-work-api").join(format!("{SESSION}.jsonl"));
        append(&transcript, "");
        let folder = transcript.with_extension("").join("subagents");
        append(
            &folder.join("agent-aui-tauri-48f322aec1fae8f7.jsonl"),
            &format!(
                "{}\n{}\n",
                claude_user("Fix the tabs", "2026-10-09T10:00:00.000Z"),
                claude_reply("Done.", Some("end_turn"), "2026-10-09T10:01:00.000Z")
            ),
        );
        fs::write(
            folder.join("agent-aui-tauri-48f322aec1fae8f7.meta.json"),
            r#"{"agentType":"general-purpose","description":"Tauri tabs","model":"sonnet","spawnDepth":1}"#,
        )
        .unwrap();
        append(&folder.join("agent-ac8b68c4f2aaa2d49.jsonl"), &format!("{}\n", claude_user("Still going", "2026-10-09T10:02:00.000Z")));

        let root = scratch.0.join(".codex/sessions");
        let mut index = Index::default();
        let listed = list(&mut index, Kind::Claude, &transcript, &root, SESSION);
        assert_eq!(listed.len(), 2);
        let done = listed.iter().find(|s| s.key == "aui-tauri-48f322aec1fae8f7").unwrap();
        assert_eq!(done.label, "Tauri tabs");
        assert_eq!(done.role, "general-purpose");
        assert_eq!(done.model.as_deref(), Some("sonnet"));
        assert_eq!(done.tokens, 10 + 5, "input and output count, cache reads do not");
        assert!(!done.busy, "its reply ended the turn");
        let busy = listed.iter().find(|s| s.key == "ac8b68c4f2aaa2d49").unwrap();
        assert!(busy.busy, "no reply has ended its turn");
        assert_eq!(busy.label, "Subagent", "a subagent with no meta file still has a label");
    }

    #[test]
    fn a_claude_view_keeps_the_task_the_tools_and_the_newest_entries() {
        let scratch = Scratch::new("claude-view");
        let transcript = scratch.0.join("projects/-work-api").join(format!("{SESSION}.jsonl"));
        append(&transcript, "");
        let folder = transcript.with_extension("").join("subagents");
        let mut lines = vec![claude_user("Fix the tabs", "2026-10-09T10:00:00.000Z")];
        lines.push(
            r#"{"type":"assistant","isSidechain":true,"timestamp":"2026-10-09T10:00:01.000Z","message":{"id":"t1","model":"claude-sonnet-5-5","stop_reason":"tool_use","content":[{"type":"tool_use","id":"u1","name":"Read","input":{"file_path":"/work/app/tabs.ts"}}]}}"#.to_string(),
        );
        lines.push(
            r#"{"type":"user","isSidechain":true,"timestamp":"2026-10-09T10:00:02.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"u1","content":[{"type":"text","text":"const tabs = []"}]}]}}"#.to_string(),
        );
        for n in 0..305 {
            lines.push(claude_reply(&format!("reply {n}"), None, "2026-10-09T10:01:00.000Z"));
        }
        append(&folder.join("agent-tabs1.jsonl"), &format!("{}\n", lines.join("\n")));

        let root = scratch.0.join(".codex/sessions");
        let mut index = Index::default();
        let view = view(&mut index, Kind::Claude, &transcript, &root, SESSION, "tabs1").unwrap();
        // The task, the tool call and its result, and 305 replies make 308 entries in all.
        assert_eq!(view.total, 308);
        assert_eq!(view.entries.len(), ENTRY_WINDOW + 1, "the task stays first, then the newest 300");
        assert_eq!(view.entries[0].role, "user");
        assert_eq!(view.entries[0].text, "Fix the tabs");
        assert_eq!(view.entries.last().unwrap().text, "reply 304");
        assert!(view.subagent.busy);
    }

    #[test]
    fn a_key_that_is_not_a_plain_id_names_no_file() {
        let scratch = Scratch::new("claude-key");
        let transcript = scratch.0.join("projects/-work-api").join(format!("{SESSION}.jsonl"));
        append(&transcript, "");
        let root = scratch.0.join(".codex/sessions");
        let mut index = Index::default();
        assert!(view(&mut index, Kind::Claude, &transcript, &root, SESSION, "../../secret").is_none());
        assert!(view(&mut index, Kind::Claude, &transcript, &root, SESSION, "").is_none());
    }

    #[test]
    fn codex_subagents_are_found_through_their_parent_at_any_depth() {
        let scratch = Scratch::new("codex-tree");
        let root = scratch.0.join(".codex/sessions");
        let day = root.join("2026/10/09");
        let parent = day.join(format!("rollout-2026-10-09T09-00-00-{PARENT}.jsonl"));
        append(&parent, &format!("{}\n", r#"{"timestamp":"2026-10-09T09:00:00Z","type":"session_meta","payload":{"id":"01a0fd3b-8f6e-7191-a8d1-15e905f89233","source":"cli","cwd":"/work/app"}}"#));
        append(&parent, &format!("{}\n", codex_tokens("2026-10-09T09:05:00Z", 99_999, 0, 1)));

        let child = day.join(format!("rollout-2026-10-09T10-00-00-{CHILD}.jsonl"));
        append(
            &child,
            &format!(
                "{}\n{}\n{}\n{}\n{}\n",
                codex_head(CHILD, Some(PARENT), 1, "Dalton", "2026-10-09T10:00:00Z"),
                codex_event("task_started", "2026-10-09T10:00:01Z"),
                codex_prompt("Review the router", "2026-10-09T10:00:02Z"),
                codex_tokens("2026-10-09T10:01:00Z", 1000, 400, 200),
                codex_event("task_complete", "2026-10-09T10:02:00Z"),
            ),
        );
        // A grandchild started by the child, in a later day folder.
        let grand = root.join("2026/10/10").join(format!("rollout-2026-10-10T08-00-00-{GRANDCHILD}.jsonl"));
        append(
            &grand,
            &format!(
                "{}\n{}\n",
                codex_head(GRANDCHILD, Some(CHILD), 2, "Ada", "2026-10-10T08:00:00Z"),
                codex_event("task_started", "2026-10-10T08:00:01Z"),
            ),
        );
        // A subagent of some other session: never listed under this one.
        let other = day.join(format!("rollout-2026-10-09T11-00-00-{OTHER}.jsonl"));
        append(&other, &format!("{}\n", codex_head(OTHER, Some("ffffffff-0000-4000-8000-000000000000"), 1, "Sam", "2026-10-09T11:00:00Z")));

        let mut index = Index::default();
        let listed = list(&mut index, Kind::Codex, &parent, &root, PARENT);
        let keys: Vec<&str> = listed.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, vec![CHILD, GRANDCHILD]);
        assert_eq!(listed[0].label, "Dalton");
        assert_eq!(listed[0].parent, None, "the session started it");
        assert!(!listed[0].busy, "it finished its task");
        assert_eq!(listed[0].tokens, 600 + 200);
        assert_eq!(listed[1].parent.as_deref(), Some(CHILD));
        assert_eq!(listed[1].depth, 2);
        assert!(listed[1].busy, "its task has started and not ended");

        let counted = totals(&mut index, Kind::Codex, &parent, &root, PARENT);
        assert_eq!(counted, Totals { count: 2, tokens: 800 });
    }

    #[test]
    fn a_codex_view_reads_its_prompt_calls_and_results_as_text() {
        let scratch = Scratch::new("codex-view");
        let root = scratch.0.join(".codex/sessions");
        let day = root.join("2026/10/09");
        let parent = day.join(format!("rollout-2026-10-09T09-00-00-{PARENT}.jsonl"));
        append(&parent, "");
        let child = day.join(format!("rollout-2026-10-09T10-00-00-{CHILD}.jsonl"));
        let call = r#"{"timestamp":"2026-10-09T10:00:03Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"ls\"}","call_id":"c1"}}"#;
        let output = r#"{"timestamp":"2026-10-09T10:00:04Z","type":"response_item","payload":{"type":"function_call_output","call_id":"c1","output":"<img src=x onerror=alert(1)>"}}"#;
        append(
            &child,
            &format!("{}\n{}\n{}\n{}\n", codex_head(CHILD, Some(PARENT), 1, "Dalton", "2026-10-09T10:00:00Z"), codex_prompt("Check the router", "2026-10-09T10:00:02Z"), call, output),
        );

        let mut index = Index::default();
        let view = view(&mut index, Kind::Codex, &parent, &root, PARENT, CHILD).unwrap();
        let roles: Vec<&str> = view.entries.iter().map(|e| e.role).collect();
        assert_eq!(roles, vec!["user", "tool", "result"]);
        assert_eq!(view.entries[0].text, "Check the router");
        assert_eq!(view.entries[1].name, "exec_command");
        assert_eq!(view.entries[1].text, r#"{"cmd":"ls"}"#);
        assert_eq!(view.entries[2].text, "<img src=x onerror=alert(1)>", "output is kept as plain text, not parsed");
    }

    #[test]
    fn long_text_is_cut_with_an_ellipsis() {
        let text = "x".repeat(TEXT_CHARS + 10);
        let cut = clip(&text, TEXT_CHARS);
        assert_eq!(cut.chars().count(), TEXT_CHARS + 1);
        assert!(cut.ends_with('…'));
        assert_eq!(clip("short", TEXT_CHARS), "short");
    }
}
