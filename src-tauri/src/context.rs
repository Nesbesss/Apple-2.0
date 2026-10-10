//! How much of its context window each session has left: the figure on each card, and the one the top bar shows for
//! the focused session.
//!
//! Claude: Claude Code's status line knows the window and the input of the last call for every session. Once Nesa's
//! status line is connected, its script writes both to ~/.claude/nesa-context/<session id>.json, and that is exact.
//! Without it, the last main-chain call in the transcript gives the input, and the window is assumed: 200K, or 1M once
//! the session has gone past 200K. An extended window below 200K reads too low until the status line is connected.
//!
//! Codex: each token report carries the model's window and the input of its last call, so both are exact.
//!
//! The figure is the input of the last call, which is what Claude's own `used_percentage` counts. Output is left out.

use crate::transcripts::is_valid_id;
use serde::Serialize;
use serde_json::Value;
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub const CLAUDE_WINDOW: u64 = 200_000;
pub const CLAUDE_EXTENDED_WINDOW: u64 = 1_000_000;
/// Where the status-line script writes each session's window and input, inside the Claude folder.
pub const CONTEXT_DIR: &str = "nesa-context";
/// Bytes read from the end of a transcript. The last call's usage is near the end, but a tool result can be long.
const TAIL_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// Input tokens of the session's last call: the context in use now.
    pub used: u64,
    /// The window the session has room for, in tokens.
    pub window: u64,
    /// False when the window is assumed. Only the status line can settle it for Claude.
    pub exact: bool,
}

/// What Claude's status line wrote for one session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClaudeReport {
    pub window: u64,
    pub input: u64,
    /// Unix millis when the script wrote it.
    pub captured_ms: u64,
}

/// The window assumed for a Claude session with no status-line report: the default, or the extended window once the
/// session has used more than the default holds.
pub fn assumed_window(used: u64) -> u64 {
    if used > CLAUDE_WINDOW {
        CLAUDE_EXTENDED_WINDOW
    } else {
        CLAUDE_WINDOW
    }
}

/// The status-line report for a session, or None when Nesa's status line has not written one.
pub fn claude_report(claude_root: &Path, session_id: &str) -> Option<ClaudeReport> {
    if !is_valid_id(session_id) {
        return None;
    }
    let text = fs::read_to_string(claude_root.join(CONTEXT_DIR).join(format!("{session_id}.json"))).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    Some(ClaudeReport {
        window: value["contextWindowSize"].as_u64().filter(|w| *w > 0)?,
        input: value["inputTokens"].as_u64()?,
        captured_ms: value["capturedAtMs"].as_u64().unwrap_or(0),
    })
}

/// The context a Claude session has now. `transcript_input` is the input of its last main-chain call, from a transcript
/// last written at `transcript_ms`. A report written after that call is newer, and it is used as it is.
pub fn claude_context(transcript_input: Option<u64>, transcript_ms: u64, report: Option<ClaudeReport>) -> Option<Context> {
    let used = match (report, transcript_input) {
        (Some(r), Some(input)) if r.captured_ms < transcript_ms => input,
        (Some(r), _) => r.input,
        (None, Some(input)) => input,
        (None, None) => return None,
    };
    Some(match report {
        Some(r) => Context { used, window: r.window, exact: true },
        None => Context { used, window: assumed_window(used), exact: false },
    })
}

/// The input of a Claude transcript's last main-chain call: the prompt the context holds now. Subagent lines and
/// synthetic lines, which make no real call, are skipped.
pub fn claude_transcript_input(path: &Path) -> Option<u64> {
    read_tail(path, TAIL_BYTES)?
        .lines()
        .rev()
        .filter(|line| line.contains("\"usage\""))
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value["isSidechain"] != true && value["message"]["model"] != "<synthetic>")
        .find_map(|value| prompt_of(&value["message"]))
}

/// The prompt of one reply. A reply that made several calls lists each in `iterations`, the advisor's calls included,
/// and the top-level counts add them all up. The context is the last call the main model made.
fn prompt_of(message: &Value) -> Option<u64> {
    let usage = &message["usage"];
    if !usage.is_object() {
        return None;
    }
    let call = usage["iterations"]
        .as_array()
        .and_then(|calls| calls.iter().rev().find(|call| call["type"] == "message"))
        .unwrap_or(usage);
    let field = |name: &str| call[name].as_u64().unwrap_or(0);
    Some(field("input_tokens") + field("cache_creation_input_tokens") + field("cache_read_input_tokens"))
}

/// The newest token report in a Codex rollout that carries totals: the info object with its running total and its last
/// call. None when the rollout has none yet.
pub fn codex_last_info(path: &Path) -> Option<Value> {
    read_tail(path, TAIL_BYTES)?
        .lines()
        .rev()
        .filter(|line| line.contains("\"token_count\""))
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .map(|value| value["payload"]["info"].clone())
        .find(|info| info["total_token_usage"].is_object())
}

/// A Codex usage object as a session total: input that was not cached, plus output. Cached input is left out, as
/// Claude's cache reads are.
pub fn codex_total(usage: &Value) -> Option<u64> {
    if !usage.is_object() {
        return None;
    }
    let input = usage["input_tokens"].as_u64().unwrap_or(0);
    let cached = usage["cached_input_tokens"].as_u64().unwrap_or(0);
    Some(input.saturating_sub(cached) + usage["output_tokens"].as_u64().unwrap_or(0))
}

/// The context a Codex session has now: the input of its last call, against the window its model reports.
pub fn codex_context(path: &Path) -> Option<Context> {
    let info = codex_last_info(path)?;
    Some(Context {
        used: info["last_token_usage"]["input_tokens"].as_u64()?,
        window: info["model_context_window"].as_u64().filter(|w| *w > 0)?,
        exact: true,
    })
}

/// The last `bytes` of a file as text. The first line is dropped when the read began inside the file, since it may be cut.
fn read_tail(path: &Path, bytes: u64) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let from = len.saturating_sub(bytes);
    file.seek(SeekFrom::Start(from)).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    Some(if from > 0 {
        text.split_once('\n').map_or(String::new(), |(_, rest)| rest.to_string())
    } else {
        text.into_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, path::PathBuf};
    use uuid::Uuid;

    /// A scratch folder that is removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("nesa-context-{}-{}", Uuid::new_v4(), name));
            fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_lines(path: &Path, lines: &[String]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut file = fs::File::create(path).unwrap();
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
    }

    /// An assistant line as Claude Code writes it, with the usage object given as JSON text.
    fn reply(model: &str, sidechain: bool, usage: &str) -> String {
        format!(
            r#"{{"type":"assistant","isSidechain":{sidechain},"timestamp":"2026-10-09T10:00:00.000Z","message":{{"id":"m","model":"{model}","usage":{usage}}}}}"#
        )
    }

    fn codex_tokens(ts: &str, input: u64, cached: u64, output: u64, last_input: u64, window: u64) -> String {
        format!(
            r#"{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":{input},"cached_input_tokens":{cached},"output_tokens":{output}}},"last_token_usage":{{"input_tokens":{last_input},"cached_input_tokens":0,"output_tokens":10}},"model_context_window":{window}}}}}}}"#
        )
    }

    #[test]
    fn the_last_main_call_is_the_context_and_sidechain_and_synthetic_lines_are_skipped() {
        let scratch = Scratch::new("last-call");
        let file = scratch.0.join("session.jsonl");
        write_lines(
            &file,
            &[
                reply("claude-opus-5-5", false, r#"{"input_tokens":5,"cache_creation_input_tokens":100,"cache_read_input_tokens":1000,"output_tokens":9}"#),
                reply("claude-opus-5-5", true, r#"{"input_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":999999,"output_tokens":9}"#),
                reply("<synthetic>", false, r#"{"input_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":0}"#),
            ],
        );
        assert_eq!(claude_transcript_input(&file), Some(5 + 100 + 1000));
    }

    #[test]
    fn a_reply_with_several_calls_counts_its_last_main_model_call_not_the_sum() {
        let scratch = Scratch::new("iterations");
        let file = scratch.0.join("session.jsonl");
        let usage = r#"{"input_tokens":4,"cache_creation_input_tokens":3929,"cache_read_input_tokens":1506952,"output_tokens":4265,"iterations":[{"type":"message","input_tokens":2,"cache_creation_input_tokens":2574,"cache_read_input_tokens":18449},{"type":"advisor_message","input_tokens":50733,"cache_creation_input_tokens":0,"cache_read_input_tokens":0},{"type":"message","input_tokens":2,"cache_creation_input_tokens":29558,"cache_read_input_tokens":21023}]}"#;
        write_lines(&file, &[reply("claude-haiku-5-5", false, usage)]);
        assert_eq!(claude_transcript_input(&file), Some(2 + 29558 + 21023));
    }

    #[test]
    fn a_claude_window_is_assumed_at_200k_and_grows_past_it() {
        assert_eq!(claude_context(Some(150_000), 0, None), Some(Context { used: 150_000, window: 200_000, exact: false }));
        assert_eq!(claude_context(Some(300_000), 0, None), Some(Context { used: 300_000, window: 1_000_000, exact: false }));
        assert_eq!(claude_context(None, 0, None), None);
    }

    #[test]
    fn a_status_line_report_gives_the_exact_window_and_the_newer_input() {
        let report = ClaudeReport { window: 1_000_000, input: 150_000, captured_ms: 2_000 };
        // The report came after the transcript's last call, so its input is the current one.
        assert_eq!(
            claude_context(Some(120_000), 1_000, Some(report)),
            Some(Context { used: 150_000, window: 1_000_000, exact: true })
        );
        // The transcript was written after the report, so the transcript's call is the current one. The window is still exact.
        assert_eq!(
            claude_context(Some(180_000), 3_000, Some(report)),
            Some(Context { used: 180_000, window: 1_000_000, exact: true })
        );
    }

    #[test]
    fn the_status_line_report_is_read_only_for_a_session_id() {
        let scratch = Scratch::new("report");
        let folder = scratch.0.join(CONTEXT_DIR);
        let id = "2e23b561-73b2-4975-8149-fc9fd89ddf39";
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(format!("{id}.json")), r#"{"contextWindowSize":1000000,"inputTokens":42,"capturedAtMs":7}"#).unwrap();
        assert_eq!(
            claude_report(&scratch.0, id),
            Some(ClaudeReport { window: 1_000_000, input: 42, captured_ms: 7 })
        );
        assert_eq!(claude_report(&scratch.0, "../etc/passwd"), None, "an id that is not a UUID names no file");
    }

    #[test]
    fn a_codex_session_uses_its_newest_last_call_and_its_window() {
        let scratch = Scratch::new("codex");
        let rollout = scratch.0.join("rollout.jsonl");
        write_lines(
            &rollout,
            &[
                r#"{"timestamp":"2026-10-09T10:00:00Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":null}}"#.into(),
                codex_tokens("2026-10-09T10:01:00Z", 1000, 400, 200, 700, 258_400),
            ],
        );
        assert_eq!(codex_context(&rollout), Some(Context { used: 700, window: 258_400, exact: true }));

        write_lines(&rollout, &[codex_tokens("2026-10-09T10:05:00Z", 5000, 4000, 500, 5000, 258_400)]);
        assert_eq!(codex_context(&rollout), Some(Context { used: 5000, window: 258_400, exact: true }), "the newest report wins");
    }

    #[test]
    fn a_codex_total_leaves_cached_input_out() {
        let usage = serde_json::json!({ "input_tokens": 1000, "cached_input_tokens": 400, "output_tokens": 200 });
        assert_eq!(codex_total(&usage), Some(800));
        assert_eq!(codex_total(&Value::Null), None);
    }
}
