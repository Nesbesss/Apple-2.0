//! Past Claude and Codex sessions on disk, so the deck can resume them. Transcripts can run to
//! tens of megabytes, so only the head of each file is read.

use crate::store::Kind;
use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

/// Transcripts shown in the resume list.
pub const LIST_LIMIT: usize = 60;
/// Bytes read from the start of a transcript. That covers the metadata and the first prompts.
const HEAD_BYTES: u64 = 256 * 1024;
/// Titles are at most this many characters, ellipsis included. It is the store's name limit.
const TITLE_CHARS: usize = 80;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Transcript {
    pub kind: Kind,
    /// The agent's own session id. Resuming hands it to the agent.
    pub id: String,
    pub cwd: PathBuf,
    /// Empty when the transcript has no readable prompt yet.
    pub title: String,
    /// Unix millis of the last write.
    pub updated: u64,
}

/// A transcript file found by its name and modification time, before its head is read.
struct Candidate {
    kind: Kind,
    id: String,
    path: PathBuf,
    updated: u64,
}

/// The newest transcripts across both agents, newest first. Folders that no longer exist are left out.
pub fn recent(limit: usize) -> Vec<Transcript> {
    let (claude, codex) = roots();
    recent_in(&claude, &codex, limit)
}

/// One transcript by kind and agent id, or None when it is not on disk.
pub fn find(kind: Kind, id: &str) -> Option<Transcript> {
    let (claude, codex) = roots();
    find_in(&claude, &codex, kind, id)
}

/// Agent session ids are UUIDs. Checking the shape keeps an id from ever naming a path.
pub fn is_valid_id(id: &str) -> bool {
    id.len() == 36 && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}

fn roots() -> (PathBuf, PathBuf) {
    let home = dirs::home_dir().unwrap_or_default();
    (home.join(".claude/projects"), home.join(".codex/sessions"))
}

fn recent_in(claude: &Path, codex: &Path, limit: usize) -> Vec<Transcript> {
    let mut found = candidates(claude, codex);
    found.sort_by(|a, b| b.updated.cmp(&a.updated));
    found.into_iter().filter_map(read).take(limit).collect()
}

fn find_in(claude: &Path, codex: &Path, kind: Kind, id: &str) -> Option<Transcript> {
    candidates(claude, codex)
        .into_iter()
        .find(|c| c.kind == kind && c.id == id)
        .and_then(read)
}

/// Every transcript under both roots, found from file names and modification times alone.
fn candidates(claude: &Path, codex: &Path) -> Vec<Candidate> {
    let mut out = Vec::new();

    // Claude: projects/<encoded cwd>/<session id>.jsonl. Subagent transcripts sit deeper and are skipped.
    for project in entries(claude).filter(|e| e.path().is_dir()) {
        for file in entries(&project.path()) {
            let name = file.file_name();
            let Some(id) = name.to_str().and_then(|n| n.strip_suffix(".jsonl")) else { continue };
            if is_valid_id(id) {
                out.push(Candidate { kind: Kind::Claude, id: id.into(), path: file.path(), updated: updated_millis(&file.path()) });
            }
        }
    }

    // Codex: sessions/YYYY/MM/DD/rollout-<timestamp>-<session id>.jsonl
    for path in codex_rollouts(codex) {
        let Some(id) = path.file_name().and_then(|n| n.to_str()).and_then(codex_id) else { continue };
        out.push(Candidate { kind: Kind::Codex, id: id.into(), updated: updated_millis(&path), path, });
    }
    out
}

fn entries(dir: &Path) -> impl Iterator<Item = fs::DirEntry> {
    fs::read_dir(dir).into_iter().flatten().flatten()
}

fn codex_rollouts(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = pending.pop() {
        for entry in entries(&dir) {
            let path = entry.path();
            if path.is_dir() {
                if depth < 3 {
                    pending.push((path, depth + 1));
                }
            } else if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("rollout-") && n.ends_with(".jsonl")) {
                files.push(path);
            }
        }
    }
    files
}

/// The session id at the end of a rollout file name. Codex ids are 36-character UUIDs.
fn codex_id(file_name: &str) -> Option<&str> {
    let stem = file_name.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
    let id = stem.get(stem.len().checked_sub(36)?..)?;
    is_valid_id(id).then_some(id)
}

fn updated_millis(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
}

/// Reads a transcript's head and keeps it only when its folder still exists.
fn read(candidate: Candidate) -> Option<Transcript> {
    let lines = head_lines(&candidate.path);
    let text = lines.iter().map(String::as_str);
    let (cwd, title) = match candidate.kind {
        Kind::Claude => {
            let head = parse_claude(text);
            (head.cwd?, head.title)
        }
        Kind::Codex => {
            let head = parse_codex(text)?;
            (head.cwd?, head.title)
        }
    };
    cwd.is_dir().then(|| Transcript {
        kind: candidate.kind,
        id: candidate.id,
        cwd,
        title: title.unwrap_or_default(),
        updated: candidate.updated,
    })
}

/// The first lines of a file, read up to `HEAD_BYTES`. A line cut off by the limit fails to parse later.
fn head_lines(path: &Path) -> Vec<String> {
    let Ok(file) = fs::File::open(path) else { return Vec::new() };
    let mut buf = Vec::new();
    if file.take(HEAD_BYTES).read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    String::from_utf8_lossy(&buf).lines().map(str::to_owned).collect()
}

#[derive(Debug, Default, PartialEq)]
struct ClaudeHead {
    cwd: Option<PathBuf>,
    title: Option<String>,
}

/// Claude lines carry `cwd`, and a rename writes a `custom-title` line. The latest name wins.
/// Without a name, the first real prompt is the title.
fn parse_claude<'a>(lines: impl IntoIterator<Item = &'a str>) -> ClaudeHead {
    let (mut cwd, mut custom, mut named, mut prompt) = (None, None, None, None);
    for line in lines {
        let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
        if cwd.is_none() {
            cwd = value["cwd"].as_str().map(PathBuf::from);
        }
        match value["type"].as_str() {
            Some("custom-title") => custom = value["customTitle"].as_str().and_then(tidy).or(custom),
            Some("agent-name") => named = value["agentName"].as_str().and_then(tidy).or(named),
            Some("user") if prompt.is_none() && !value["isSidechain"].as_bool().unwrap_or(false) => {
                prompt = prompt_text(&value["message"]["content"]);
            }
            _ => {}
        }
    }
    ClaudeHead { cwd, title: custom.or(named).or(prompt) }
}

#[derive(Debug, Default, PartialEq)]
struct CodexHead {
    cwd: Option<PathBuf>,
    title: Option<String>,
}

/// Codex rollouts open with a `session_meta` line. Subagent rollouts are not sessions a person
/// resumes, so they return None, as do files that do not open that way.
fn parse_codex<'a>(lines: impl IntoIterator<Item = &'a str>) -> Option<CodexHead> {
    let mut lines = lines.into_iter();
    let meta: Value = serde_json::from_str(lines.next()?).ok()?;
    if meta["type"] != "session_meta" || meta["payload"]["source"].is_object() {
        return None;
    }
    let title = lines
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v["type"] == "response_item" && v["payload"]["type"] == "message" && v["payload"]["role"] == "user")
        .find_map(|v| prompt_text(&v["payload"]["content"]));
    Some(CodexHead { cwd: meta["payload"]["cwd"].as_str().map(PathBuf::from), title })
}

/// The text of a user message: a plain string, or its first text block. Markup such as `<command-name>`
/// or Codex's environment context is not a prompt, so it is skipped.
fn prompt_text(content: &Value) -> Option<String> {
    let text = match content {
        Value::String(s) => s.as_str(),
        Value::Array(blocks) => blocks
            .iter()
            .find(|b| b["type"] == "text" || b["type"] == "input_text")?["text"]
            .as_str()?,
        _ => return None,
    };
    if text.trim_start().starts_with('<') || text.trim_start().starts_with("# AGENTS.md") {
        return None;
    }
    tidy(text)
}

/// Collapses whitespace and cuts the text to `TITLE_CHARS`. None when nothing is left.
fn tidy(text: &str) -> Option<String> {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        return None;
    }
    let mut chars = flat.chars();
    let short: String = chars.by_ref().take(TITLE_CHARS - 1).collect();
    Some(if chars.next().is_some() { format!("{short}…") } else { short })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};
    use uuid::Uuid;

    const CLAUDE_ID: &str = "2e23b561-73b2-4975-8149-fc9fd89ddf39";
    const CODEX_ID: &str = "01a0fd3b-8f6e-7191-a8d1-15e905f89233";

    /// A fresh pair of roots under the system temp dir, removed when the test ends.
    struct Roots {
        base: PathBuf,
    }

    impl Roots {
        fn new() -> Self {
            let base = std::env::temp_dir().join(format!("nesa-transcripts-{}", Uuid::new_v4()));
            fs::create_dir_all(base.join("claude")).unwrap();
            fs::create_dir_all(base.join("codex")).unwrap();
            Roots { base }
        }
        fn claude(&self) -> PathBuf {
            self.base.join("claude")
        }
        fn codex(&self) -> PathBuf {
            self.base.join("codex")
        }
        fn folder(&self, name: &str) -> PathBuf {
            let dir = self.base.join(name);
            fs::create_dir_all(&dir).unwrap();
            dir
        }
        /// Writes a Claude transcript for `id` under a project folder, and sets its modification time.
        fn claude_file(&self, id: &str, lines: &[String], mtime: u64) -> PathBuf {
            let dir = self.claude().join("-project");
            fs::create_dir_all(&dir).unwrap();
            let path = dir.join(format!("{id}.jsonl"));
            fs::write(&path, lines.join("\n")).unwrap();
            set_mtime(&path, mtime);
            path
        }
        /// Writes a Codex rollout under the date folders, and sets its modification time.
        fn codex_file(&self, id: &str, lines: &[String], mtime: u64) -> PathBuf {
            let dir = self.codex().join("2026/10/02");
            fs::create_dir_all(&dir).unwrap();
            let path = dir.join(format!("rollout-2026-10-02T17-28-54-{id}.jsonl"));
            fs::write(&path, lines.join("\n")).unwrap();
            set_mtime(&path, mtime);
            path
        }
    }

    impl Drop for Roots {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn set_mtime(path: &Path, secs: u64) {
        let time = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
        fs::File::options().write(true).open(path).unwrap().set_modified(time).unwrap();
    }

    fn claude_prompt(cwd: &Path, text: &str) -> String {
        serde_json::json!({
            "type": "user", "cwd": cwd, "sessionId": CLAUDE_ID,
            "message": { "role": "user", "content": text }
        })
        .to_string()
    }

    fn codex_meta(cwd: &Path, source: Value) -> String {
        serde_json::json!({
            "type": "session_meta",
            "payload": { "id": CODEX_ID, "cwd": cwd, "source": source }
        })
        .to_string()
    }

    fn codex_prompt(text: &str) -> String {
        serde_json::json!({
            "type": "response_item",
            "payload": { "type": "message", "role": "user", "content": [{ "type": "input_text", "text": text }] }
        })
        .to_string()
    }

    #[test]
    fn claude_custom_title_wins_over_the_first_prompt() {
        let lines = [
            r#"{"type":"custom-title","customTitle":"Fix login","sessionId":"x"}"#,
            r#"{"type":"custom-title","customTitle":"Fix the login flow","sessionId":"x"}"#,
            r#"{"type":"user","cwd":"/work/app","message":{"role":"user","content":"Something else"}}"#,
        ];
        let head = parse_claude(lines);
        assert_eq!(head.title.as_deref(), Some("Fix the login flow"));
        assert_eq!(head.cwd, Some(PathBuf::from("/work/app")));
    }

    #[test]
    fn claude_skips_command_markup_and_sidechain_prompts() {
        let lines = [
            r#"{"type":"user","cwd":"/work/app","message":{"role":"user","content":"<command-name>/clear</command-name>"}}"#,
            r#"{"type":"user","isSidechain":true,"message":{"role":"user","content":"sub task"}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"  Add   tests\nfor the parser  "}]}}"#,
        ];
        assert_eq!(parse_claude(lines).title.as_deref(), Some("Add tests for the parser"));
    }

    #[test]
    fn claude_head_ignores_a_line_cut_off_by_the_read_limit() {
        let lines = [
            r#"{"type":"user","cwd":"/work/app","message":{"role":"user","content":"Hello"}}"#,
            r#"{"type":"user","cwd":"/work/ap"#,
        ];
        let head = parse_claude(lines);
        assert_eq!(head.cwd, Some(PathBuf::from("/work/app")));
        assert_eq!(head.title.as_deref(), Some("Hello"));
    }

    #[test]
    fn long_titles_are_cut_with_an_ellipsis() {
        let long = "word ".repeat(40);
        let title = tidy(&long).unwrap();
        assert_eq!(title.chars().count(), TITLE_CHARS);
        assert!(title.ends_with('…'));
        assert_eq!(tidy("   "), None);
    }

    #[test]
    fn codex_subagent_rollouts_are_not_sessions() {
        let lines = [codex_meta(Path::new("/work/app"), serde_json::json!({ "subagent": "review" }))];
        assert_eq!(parse_codex(lines.iter().map(String::as_str)), None);
    }

    #[test]
    fn codex_title_skips_environment_context() {
        let lines = [
            codex_meta(Path::new("/work/app"), Value::Null),
            codex_prompt("<environment_context>\n  <cwd>/work/app</cwd>\n</environment_context>"),
            codex_prompt("Refactor the store module"),
        ];
        let head = parse_codex(lines.iter().map(String::as_str)).unwrap();
        assert_eq!(head.cwd, Some(PathBuf::from("/work/app")));
        assert_eq!(head.title.as_deref(), Some("Refactor the store module"));
    }

    #[test]
    fn recent_lists_both_agents_newest_first_and_respects_the_limit() {
        let roots = Roots::new();
        let folder = roots.folder("app");
        let prompt = |text: &str| vec![claude_prompt(&folder, text)];
        roots.claude_file(CLAUDE_ID, &prompt("Older Claude"), 1_000);
        roots.codex_file(CODEX_ID, &[codex_meta(&folder, Value::Null), codex_prompt("Newer Codex")], 2_000);

        let all = recent_in(&roots.claude(), &roots.codex(), 10);
        assert_eq!(all.iter().map(|t| t.kind).collect::<Vec<_>>(), vec![Kind::Codex, Kind::Claude]);
        assert_eq!(all[0].title, "Newer Codex");
        assert_eq!(all[0].id, CODEX_ID);
        assert_eq!(recent_in(&roots.claude(), &roots.codex(), 1).len(), 1);
    }

    #[test]
    fn transcripts_whose_folder_is_gone_are_left_out() {
        let roots = Roots::new();
        let gone = roots.base.join("deleted-folder");
        roots.claude_file(CLAUDE_ID, &[claude_prompt(&gone, "orphan")], 1_000);
        assert!(recent_in(&roots.claude(), &roots.codex(), 10).is_empty());
    }

    #[test]
    fn find_matches_by_kind_and_id_and_returns_the_folder() {
        let roots = Roots::new();
        let folder = roots.folder("app");
        roots.codex_file(CODEX_ID, &[codex_meta(&folder, Value::Null), codex_prompt("Ship it")], 1_000);

        let found = find_in(&roots.claude(), &roots.codex(), Kind::Codex, CODEX_ID).unwrap();
        assert_eq!(found.cwd, folder);
        assert!(find_in(&roots.claude(), &roots.codex(), Kind::Claude, CODEX_ID).is_none());
        assert!(find_in(&roots.claude(), &roots.codex(), Kind::Codex, "00000000-0000-0000-0000-000000000000").is_none());
    }

    #[test]
    fn ids_must_look_like_uuids() {
        assert!(is_valid_id(CLAUDE_ID));
        assert!(!is_valid_id("../../etc/passwd"));
        assert!(!is_valid_id(""));
        assert_eq!(codex_id("rollout-2026-10-02T17-28-54-01a0fd3b-8f6e-7191-a8d1-15e905f89233.jsonl"), Some(CODEX_ID));
        assert_eq!(codex_id("notes.jsonl"), None);
    }
}
