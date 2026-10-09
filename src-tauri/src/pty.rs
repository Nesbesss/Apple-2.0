use crate::{agents, store::Session, AppState};
use base64::{engine::general_purpose::STANDARD, Engine};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::{
    io::{Read, Write},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

const DEFAULT_COLS: u16 = 100;
const DEFAULT_ROWS: u16 = 30;
/// Printed by the Codex TUI while it works ("Working (…  • esc to interrupt)").
const CODEX_BUSY_TEXT: &[u8] = b"to interrupt)";
/// Session markers the parent Claude Code process sets. A child that inherits them runs as a
/// nested session, which disables transcript saving. User configuration variables are kept.
const PARENT_SESSION_VARS: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_PID",
];

/// A running agent process behind a pseudo-terminal.
pub struct Pty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    pub exited: Arc<AtomicBool>,
    /// Unix millis of the last output chunk. Zero until the first output.
    pub last_output: Arc<AtomicU64>,
    /// Unix millis of the last chunk that contained Codex's "working" text.
    pub last_busy_text: Arc<AtomicU64>,
    pub started: SystemTime,
}

impl Pty {
    pub fn is_running(&self) -> bool {
        !self.exited.load(Ordering::Relaxed)
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.writer
            .write_all(bytes)
            .and_then(|()| self.writer.flush())
            .map_err(|e| e.to_string())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), String> {
        self.master
            .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string())
    }

    /// Hangs up the child. The waiter thread reports the exit once the process is gone.
    pub fn stop(&mut self) {
        let _ = self.killer.kill();
    }

    pub fn recently_output(&self, window: Duration) -> bool {
        let last = self.last_output.load(Ordering::Relaxed);
        last != 0 && unix_millis().saturating_sub(last) <= window.as_millis() as u64
    }

    /// True if the "working" text was printed within `window`. Used for Codex, which has no status file.
    pub fn busy_text_within(&self, window: Duration) -> bool {
        let last = self.last_busy_text.load(Ordering::Relaxed);
        last != 0 && unix_millis().saturating_sub(last) <= window.as_millis() as u64
    }
}

#[derive(Clone, Serialize)]
struct DataEvent {
    id: String,
    /// Base64 of raw terminal bytes. Base64 keeps multi-byte UTF-8 intact across chunks.
    data: String,
}

#[derive(Clone, Serialize)]
struct ExitEvent {
    id: String,
    code: Option<u32>,
}

/// Starts the agent for `session` in its folder and streams its output to the UI.
pub fn spawn(app: &AppHandle, session: &Session) -> Result<Pty, String> {
    let launch = agents::launch_for(session);
    let pair = native_pty_system()
        .openpty(PtySize { rows: DEFAULT_ROWS, cols: DEFAULT_COLS, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| e.to_string())?;

    let mut cmd = CommandBuilder::new(&launch.program);
    cmd.args(&launch.args);
    cmd.cwd(&session.cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    if std::env::var_os("LANG").is_none() {
        cmd.env("LANG", "en_US.UTF-8");
    }
    for var in PARENT_SESSION_VARS {
        cmd.env_remove(var);
    }

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Could not start {}: {e}", session.kind.label()))?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let killer = child.clone_killer();
    let exited = Arc::new(AtomicBool::new(false));
    let last_output = Arc::new(AtomicU64::new(0));
    let last_busy_text = Arc::new(AtomicU64::new(0));

    {
        let app = app.clone();
        let id = session.id.clone();
        let last_output = last_output.clone();
        let last_busy_text = last_busy_text.clone();
        thread::spawn(move || {
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let now = unix_millis();
                        last_output.store(now, Ordering::Relaxed);
                        if contains(&buf[..n], CODEX_BUSY_TEXT) {
                            last_busy_text.store(now, Ordering::Relaxed);
                        }
                        let event = DataEvent { id: id.clone(), data: STANDARD.encode(&buf[..n]) };
                        let _ = app.emit("pty-data", event);
                    }
                }
            }
        });
    }

    {
        let app = app.clone();
        let id = session.id.clone();
        let exited = exited.clone();
        let mut child = child;
        thread::spawn(move || {
            let code = child.wait().ok().map(|status| status.exit_code());
            exited.store(true, Ordering::Relaxed);
            // A restart replaces the map entry. Only report an exit for the process still registered.
            let state = app.state::<AppState>();
            let current = state
                .ptys
                .lock()
                .unwrap()
                .get(&id)
                .is_some_and(|p| Arc::ptr_eq(&p.exited, &exited));
            if current {
                let _ = app.emit("pty-exit", ExitEvent { id, code });
            }
        });
    }

    Ok(Pty {
        master: pair.master,
        writer,
        killer,
        exited,
        last_output,
        last_busy_text,
        started: SystemTime::now(),
    })
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|window| window == needle)
}

fn unix_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}
