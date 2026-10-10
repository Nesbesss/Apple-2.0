//! Dictation: a Swift sidecar (`speech-helper`) listens while the user holds the voice-mode keys.
//! The helper runs as its own process, so a speech failure can never take down a terminal
//! session. Its JSON lines are forwarded to the UI as `dictation` events.

use serde_json::{json, Value};
use std::{
    collections::HashSet,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::Mutex,
    thread,
};
use tauri::{AppHandle, Emitter, State};

use crate::AppState;

type CmdResult<T> = Result<T, String>;

/// Most vocabulary words the engine is given for one hold. More than this bias it too far.
const MAX_WORDS: usize = 100;
/// Longest word or phrase the engine is given.
const MAX_WORD_CHARS: usize = 80;

struct Helper {
    child: Child,
    stdin: ChildStdin,
}

/// The running helper, started on first use and restarted if it exits.
#[derive(Default)]
pub struct Dictation {
    helper: Mutex<Option<Helper>>,
}

/// The helper is bundled next to the app binary (`externalBin` strips the target triple).
fn helper_path() -> CmdResult<PathBuf> {
    let exe = std::env::current_exe().map_err(|e| format!("Could not find the app: {e}"))?;
    let path = exe.with_file_name("speech-helper");
    if path.exists() {
        Ok(path)
    } else {
        Err("The speech helper is missing from this build".into())
    }
}

fn spawn(app: &AppHandle) -> CmdResult<Helper> {
    let mut child = Command::new(helper_path()?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Could not start the speech helper: {e}"))?;
    let stdin = child.stdin.take().ok_or("The speech helper has no input")?;
    let stdout = child.stdout.take().ok_or("The speech helper has no output")?;

    let app = app.clone();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(event) = serde_json::from_str::<Value>(&line) {
                let _ = app.emit("dictation", event);
            }
        }
        let _ = app.emit("dictation", json!({ "event": "exited" }));
    });
    Ok(Helper { child, stdin })
}

fn ensure_running(slot: &mut Option<Helper>, app: &AppHandle) -> CmdResult<()> {
    // A helper that has exited (a crash, or the system stopped it) is replaced, not reused.
    let exited = matches!(slot.as_mut().map(|h| h.child.try_wait()), Some(Ok(Some(_))));
    if exited {
        *slot = None;
    }
    if slot.is_none() {
        *slot = Some(spawn(app)?);
    }
    Ok(())
}

/// Starts the helper at launch. It prepares the language model and the audio engine while the
/// app is idle, so the first hold starts listening within about 100 ms.
pub fn warm(app: &AppHandle, state: &AppState) -> CmdResult<()> {
    ensure_running(&mut state.dictation.helper.lock().unwrap(), app)
}

/// Trims the words, drops blanks and overlong ones, and keeps the first of any that repeat ignoring
/// case. The engine never gets more than `MAX_WORDS`.
fn clean_words(words: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    words
        .into_iter()
        .map(|word| word.trim().to_string())
        .filter(|word| !word.is_empty() && word.chars().count() <= MAX_WORD_CHARS)
        .filter(|word| seen.insert(word.to_lowercase()))
        .take(MAX_WORDS)
        .collect()
}

fn send(app: &AppHandle, state: &AppState, request: Value) -> CmdResult<()> {
    let mut slot = state.dictation.helper.lock().unwrap();
    ensure_running(&mut slot, app)?;
    let helper = slot.as_mut().expect("helper was just started");
    let written = writeln!(helper.stdin, "{request}").and_then(|_| helper.stdin.flush());
    if written.is_err() {
        // The helper is gone. Reap it so the next command starts a fresh one.
        if let Some(mut dead) = slot.take() {
            let _ = dead.child.kill();
            let _ = dead.child.wait();
        }
        return Err("The speech helper stopped. Try again.".into());
    }
    Ok(())
}

/// Starts listening. `words` is the user's vocabulary, and `builtin` says whether Nesa's own developer
/// words go in too. Both travel with every start, so a helper that was restarted still gets them.
#[tauri::command]
pub fn dictation_start(app: AppHandle, state: State<'_, AppState>, words: Vec<String>, builtin: bool) -> CmdResult<()> {
    send(&app, state.inner(), json!({ "cmd": "start", "words": clean_words(words), "builtin": builtin }))
}

#[tauri::command]
pub fn dictation_stop(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    send(&app, state.inner(), json!({ "cmd": "stop" }))
}

#[tauri::command]
pub fn dictation_cancel(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    send(&app, state.inner(), json!({ "cmd": "cancel" }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|word| word.to_string()).collect()
    }

    #[test]
    fn words_are_trimmed_and_blanks_dropped() {
        assert_eq!(clean_words(words(&["  Nesa ", "", "   ", "Tauri"])), words(&["Nesa", "Tauri"]));
    }

    #[test]
    fn repeats_ignoring_case_keep_the_first_spelling() {
        assert_eq!(clean_words(words(&["Claude", "claude", "CLAUDE", "Codex"])), words(&["Claude", "Codex"]));
    }

    #[test]
    fn overlong_words_are_dropped() {
        let long = "x".repeat(MAX_WORD_CHARS + 1);
        assert_eq!(clean_words(words(&[long.as_str(), "Nesa"])), words(&["Nesa"]));
    }

    #[test]
    fn the_list_is_capped() {
        let many: Vec<String> = (0..MAX_WORDS + 20).map(|i| format!("word{i}")).collect();
        let cleaned = clean_words(many);
        assert_eq!(cleaned.len(), MAX_WORDS);
        assert_eq!(cleaned[0], "word0");
    }
}
