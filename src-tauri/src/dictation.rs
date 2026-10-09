//! Dictation: a Swift sidecar (`speech-helper`) listens while the user holds Option+Command.
//! The helper runs as its own process, so a speech failure can never take down a terminal
//! session. Its JSON lines are forwarded to the UI as `dictation` events.

use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::Mutex,
    thread,
};
use tauri::{AppHandle, Emitter, State};

use crate::AppState;

type CmdResult<T> = Result<T, String>;

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

fn send(app: &AppHandle, state: &AppState, cmd: &str) -> CmdResult<()> {
    let mut slot = state.dictation.helper.lock().unwrap();
    ensure_running(&mut slot, app)?;
    let helper = slot.as_mut().expect("helper was just started");
    let written = writeln!(helper.stdin, r#"{{"cmd":"{cmd}"}}"#).and_then(|_| helper.stdin.flush());
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

#[tauri::command]
pub fn dictation_start(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    send(&app, state.inner(), "start")
}

#[tauri::command]
pub fn dictation_stop(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    send(&app, state.inner(), "stop")
}

#[tauri::command]
pub fn dictation_cancel(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    send(&app, state.inner(), "cancel")
}
