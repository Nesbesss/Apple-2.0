//! Claude's own 5-hour and 7-day limits, read from the status line that Claude Code runs.
//!
//! Claude Code passes the account's rate-limit windows to its status-line command. That is the same
//! data `/usage` shows, and Claude Code supplies it itself, so Nesa never reads or sends the login
//! credentials. To connect, Nesa writes a small script (assets/nesa-statusline.py) to ~/.claude and sets
//! `statusLine` in ~/.claude/settings.json. The script keeps the latest windows in ~/.claude/nesa-usage.json.
//! Connecting changes a file the user owns, so the frontend asks first, and the original settings are
//! kept as a backup.

use crate::usage::{window_at, Window};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

const SCRIPT: &str = include_str!("../assets/nesa-statusline.py");
const SCRIPT_NAME: &str = "nesa-statusline.py";
/// The command written to settings.json. Claude Code runs it through the shell, so `~` expands.
const COMMAND: &str = "python3 ~/.claude/nesa-statusline.py";
const REPORT_NAME: &str = "nesa-usage.json";
const SETTINGS_NAME: &str = "settings.json";
const BACKUP_NAME: &str = "settings.json.nesa-backup";
const FIVE_HOUR_MINUTES: u64 = 300;
const SEVEN_DAY_MINUTES: u64 = 10_080;

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// The status line is not Nesa's. Nothing is read.
    Off,
    /// Connected, but Claude Code has not sent a limit report yet. It sends one after the first reply.
    Waiting,
    /// Connected, with a report.
    Live,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeLimits {
    pub status: Status,
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
    /// When the script last wrote a report. Shows how current the numbers are.
    pub captured_at_ms: Option<u64>,
}

fn claude_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude"))
}

/// Whether the status line in settings.json is Nesa's command.
fn is_connected(settings: &Map<String, Value>) -> bool {
    settings.get("statusLine").is_some_and(|line| line["command"] == COMMAND)
}

/// Reads settings.json as an object. A missing or empty file is an empty object. Anything else that
/// is not an object is an error, so Nesa never overwrites a file it cannot read.
fn read_settings(path: &Path) -> Result<Map<String, Value>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(err) => return Err(format!("Could not read {}: {err}", path.display())),
    };
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err("Claude's settings.json is not a JSON object, so Nesa left it alone.".into()),
    }
}

/// Writes through a temporary file, so a failed write cannot leave settings.json half-written.
fn write_settings(path: &Path, settings: &Map<String, Value>) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(&Value::Object(settings.clone())).map_err(|e| e.to_string())?;
    text.push('\n');
    let temp = path.with_extension("json.nesa-tmp");
    fs::write(&temp, text).map_err(|e| format!("Could not write {}: {e}", temp.display()))?;
    fs::rename(&temp, path).map_err(|e| format!("Could not replace {}: {e}", path.display()))
}

/// What the usage bar needs. An unreadable report reads as waiting, not as an error.
pub fn read(root: &Path, now: u64) -> ClaudeLimits {
    let settings = read_settings(&root.join(SETTINGS_NAME)).unwrap_or_default();
    if !is_connected(&settings) {
        return ClaudeLimits { status: Status::Off, five_hour: None, seven_day: None, captured_at_ms: None };
    }
    let report: Value = fs::read_to_string(root.join(REPORT_NAME))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    let captured = report["capturedAtMs"].as_u64();
    let window = |key: &str, minutes: u64| -> Option<Window> {
        let raw = &report[key];
        let used = raw["usedPercent"].as_f64()?;
        let resets_at = raw["resetsAtMs"].as_u64()?;
        Some(window_at(used, minutes, resets_at, captured.unwrap_or(now), now))
    };
    let five_hour = window("fiveHour", FIVE_HOUR_MINUTES);
    let seven_day = window("sevenDay", SEVEN_DAY_MINUTES);
    let status = if five_hour.is_some() || seven_day.is_some() { Status::Live } else { Status::Waiting };
    ClaudeLimits { status, five_hour, seven_day, captured_at_ms: captured }
}

/// Connects the usage bar. Refuses, and changes nothing, when the status line belongs to someone else.
pub fn connect(root: &Path) -> Result<(), String> {
    let settings_path = root.join(SETTINGS_NAME);
    let mut settings = read_settings(&settings_path)?;
    if settings.get("statusLine").is_some() && !is_connected(&settings) {
        return Err("Claude Code already has a status line, so Nesa left settings.json alone. Add ~/.claude/nesa-statusline.py to that status line yourself.".into());
    }
    fs::create_dir_all(root).map_err(|e| format!("Could not create {}: {e}", root.display()))?;
    // The original is kept once. Later connects do not replace that backup.
    let backup = root.join(BACKUP_NAME);
    if settings_path.exists() && !backup.exists() {
        fs::copy(&settings_path, &backup).map_err(|e| format!("Could not back up settings.json: {e}"))?;
    }
    // The script goes first, so the status line never points at a missing file.
    fs::write(root.join(SCRIPT_NAME), SCRIPT).map_err(|e| format!("Could not write {SCRIPT_NAME}: {e}"))?;
    settings.insert("statusLine".into(), json!({ "type": "command", "command": COMMAND }));
    write_settings(&settings_path, &settings)
}

/// Removes Nesa's status line, script and report. Leaves any status line that is not Nesa's alone.
pub fn disconnect(root: &Path) -> Result<(), String> {
    let settings_path = root.join(SETTINGS_NAME);
    let mut settings = read_settings(&settings_path)?;
    if !is_connected(&settings) {
        return Ok(());
    }
    settings.remove("statusLine");
    write_settings(&settings_path, &settings)?;
    let _ = fs::remove_file(root.join(SCRIPT_NAME));
    let _ = fs::remove_file(root.join(REPORT_NAME));
    let _ = fs::remove_dir_all(root.join(crate::context::CONTEXT_DIR));
    Ok(())
}

/// Replaces Nesa's script with this build's copy when the status line is connected and the copy on disk is older.
/// Nothing else is touched, and a status line that is not Nesa's is left alone. Returns whether the script was replaced.
pub fn refresh_script(root: &Path) -> Result<bool, String> {
    let settings = read_settings(&root.join(SETTINGS_NAME))?;
    if !is_connected(&settings) {
        return Ok(false);
    }
    let path = root.join(SCRIPT_NAME);
    if fs::read_to_string(&path).is_ok_and(|current| current == SCRIPT) {
        return Ok(false);
    }
    // Written beside the script and renamed over it, so Claude Code never runs a half-written file.
    let temp = root.join(format!("{SCRIPT_NAME}.tmp"));
    fs::write(&temp, SCRIPT).map_err(|e| format!("Could not write {SCRIPT_NAME}: {e}"))?;
    fs::rename(&temp, &path).map_err(|e| format!("Could not replace {SCRIPT_NAME}: {e}"))?;
    Ok(true)
}

#[tauri::command]
pub fn claude_limits() -> Result<ClaudeLimits, String> {
    let root = claude_dir().ok_or("Could not find the home folder")?;
    Ok(read(&root, crate::usage::now_ms()))
}

#[tauri::command]
pub fn connect_claude_limits() -> Result<ClaudeLimits, String> {
    let root = claude_dir().ok_or("Could not find the home folder")?;
    connect(&root)?;
    Ok(read(&root, crate::usage::now_ms()))
}

#[tauri::command]
pub fn disconnect_claude_limits() -> Result<ClaudeLimits, String> {
    let root = claude_dir().ok_or("Could not find the home folder")?;
    disconnect(&root)?;
    Ok(read(&root, crate::usage::now_ms()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use crate::context::CONTEXT_DIR;
    use std::process::{Command, Stdio};

    /// A scratch folder that is removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("nesa-claude-{}-{}", std::process::id(), name));
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

    const NOW: u64 = 1_791_504_000_000;

    fn settings_of(root: &Path) -> Map<String, Value> {
        read_settings(&root.join(SETTINGS_NAME)).unwrap()
    }

    #[test]
    fn without_a_status_line_nothing_is_connected() {
        let scratch = Scratch::new("off");
        let limits = read(&scratch.0, NOW);
        assert_eq!(limits.status, Status::Off);
        assert!(limits.five_hour.is_none());
    }

    #[test]
    fn connecting_writes_the_script_and_the_status_line_then_waits_for_a_report() {
        let scratch = Scratch::new("connect");
        connect(&scratch.0).unwrap();
        assert!(scratch.0.join(SCRIPT_NAME).exists());
        assert_eq!(settings_of(&scratch.0)["statusLine"]["command"], COMMAND);
        assert_eq!(read(&scratch.0, NOW).status, Status::Waiting);
    }

    #[test]
    fn connecting_keeps_other_settings_and_backs_up_the_original_once() {
        let scratch = Scratch::new("keep");
        let original = "{\n  \"model\": \"opus\",\n  \"theme\": \"dark\"\n}\n";
        fs::write(scratch.0.join(SETTINGS_NAME), original).unwrap();
        connect(&scratch.0).unwrap();
        let settings = settings_of(&scratch.0);
        assert_eq!(settings["model"], "opus");
        assert_eq!(settings["theme"], "dark");
        assert_eq!(fs::read_to_string(scratch.0.join(BACKUP_NAME)).unwrap(), original);
        connect(&scratch.0).unwrap();
        assert_eq!(fs::read_to_string(scratch.0.join(BACKUP_NAME)).unwrap(), original, "the backup is not replaced");
    }

    #[test]
    fn a_status_line_that_is_not_nesas_is_left_alone() {
        let scratch = Scratch::new("foreign");
        let original = r#"{"statusLine":{"type":"command","command":"~/.claude/statusline.sh"}}"#;
        fs::write(scratch.0.join(SETTINGS_NAME), original).unwrap();
        assert!(connect(&scratch.0).is_err());
        assert_eq!(fs::read_to_string(scratch.0.join(SETTINGS_NAME)).unwrap(), original);
        assert!(!scratch.0.join(SCRIPT_NAME).exists(), "no script is written when connecting is refused");
    }

    #[test]
    fn unreadable_settings_are_never_overwritten() {
        let scratch = Scratch::new("invalid");
        fs::write(scratch.0.join(SETTINGS_NAME), "{ not json").unwrap();
        assert!(connect(&scratch.0).is_err());
        assert_eq!(fs::read_to_string(scratch.0.join(SETTINGS_NAME)).unwrap(), "{ not json");
    }

    #[test]
    fn disconnecting_removes_only_nesas_entries() {
        let scratch = Scratch::new("disconnect");
        fs::write(scratch.0.join(SETTINGS_NAME), r#"{"model":"opus"}"#).unwrap();
        connect(&scratch.0).unwrap();
        fs::write(scratch.0.join(REPORT_NAME), "{}").unwrap();
        disconnect(&scratch.0).unwrap();
        let settings = settings_of(&scratch.0);
        assert!(settings.get("statusLine").is_none());
        assert_eq!(settings["model"], "opus");
        assert!(!scratch.0.join(SCRIPT_NAME).exists());
        assert!(!scratch.0.join(REPORT_NAME).exists());
        assert_eq!(read(&scratch.0, NOW).status, Status::Off);
    }

    #[test]
    fn a_window_whose_reset_has_passed_reads_as_unused() {
        let scratch = Scratch::new("rolled");
        connect(&scratch.0).unwrap();
        let report = format!(
            r#"{{"capturedAtMs":{},"fiveHour":{{"usedPercent":88.0,"resetsAtMs":{}}},"sevenDay":{{"usedPercent":41.2,"resetsAtMs":{}}}}}"#,
            NOW - 3_600_000,
            NOW - 1_800_000,
            NOW + 86_400_000
        );
        fs::write(scratch.0.join(REPORT_NAME), report).unwrap();
        let limits = read(&scratch.0, NOW);
        assert_eq!(limits.status, Status::Live);
        let five = limits.five_hour.unwrap();
        assert_eq!(five.used_percent, 0.0, "the 88% belonged to a window that has closed");
        assert!(five.resets_at_ms > NOW);
        let week = limits.seven_day.unwrap();
        assert_eq!(week.used_percent, 41.2);
        assert_eq!(week.resets_at_ms, NOW + 86_400_000);
    }

    #[test]
    fn a_report_without_a_window_leaves_that_window_absent() {
        let scratch = Scratch::new("absent");
        connect(&scratch.0).unwrap();
        fs::write(scratch.0.join(REPORT_NAME), format!(r#"{{"capturedAtMs":{NOW},"sevenDay":{{"usedPercent":5.0,"resetsAtMs":{}}}}}"#, NOW + 1000)).unwrap();
        let limits = read(&scratch.0, NOW);
        assert!(limits.five_hour.is_none());
        assert_eq!(limits.seven_day.unwrap().used_percent, 5.0);
    }

    /// Runs the script the way Claude Code does: session JSON on stdin, and HOME set so that
    /// `~/.claude` is the folder `connect` wrote into.
    fn run_script(home: &Path, session: &str) {
        let mut child = Command::new("python3")
            .arg(home.join(".claude").join(SCRIPT_NAME))
            .env("HOME", home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("python3 is installed");
        child.stdin.take().unwrap().write_all(session.as_bytes()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert!(output.stdout.is_empty(), "the status line stays blank");
    }

    #[test]
    fn the_script_keeps_the_latest_windows_and_ignores_sessions_without_limits() {
        let scratch = Scratch::new("script");
        // HOME is the scratch folder, so ~/.claude is scratch/.claude, where connect writes.
        let claude = scratch.0.join(".claude");
        connect(&claude).unwrap();
        run_script(&scratch.0, r#"{"model":{"display_name":"Opus"}}"#);
        assert!(!claude.join(REPORT_NAME).exists(), "no limits, nothing written");

        run_script(
            &scratch.0,
            r#"{"rate_limits":{"five_hour":{"used_percentage":23.5,"resets_at":1738425600},"seven_day":{"used_percentage":41.2,"resets_at":1738857600}}}"#,
        );
        // A later report that drops the five-hour window (its reset passed) keeps the last one it had.
        run_script(&scratch.0, r#"{"rate_limits":{"seven_day":{"used_percentage":42.0,"resets_at":1738857600}}}"#);
        let report: Value = serde_json::from_str(&fs::read_to_string(claude.join(REPORT_NAME)).unwrap()).unwrap();
        assert_eq!(report["fiveHour"]["usedPercent"], 23.5);
        assert_eq!(report["fiveHour"]["resetsAtMs"], 1738425600_u64 * 1000);
        assert_eq!(report["sevenDay"]["usedPercent"], 42.0);
    }

    #[test]
    fn the_script_writes_each_sessions_window_and_last_input_and_skips_ids_that_are_not_uuids() {
        let scratch = Scratch::new("context-script");
        let claude = scratch.0.join(".claude");
        connect(&claude).unwrap();
        let session_id = "2e23b561-73b2-4975-8149-fc9fd89ddf39";
        run_script(
            &scratch.0,
            &format!(
                r#"{{"session_id":"{session_id}","context_window":{{"context_window_size":1000000,"current_usage":{{"input_tokens":2,"cache_creation_input_tokens":3000,"cache_read_input_tokens":40000,"output_tokens":9}}}}}}"#
            ),
        );
        let record: Value = serde_json::from_str(&fs::read_to_string(claude.join(CONTEXT_DIR).join(format!("{session_id}.json"))).unwrap()).unwrap();
        assert_eq!(record["contextWindowSize"], 1_000_000);
        assert_eq!(record["inputTokens"], 43_002, "output is left out, as Claude's used_percentage leaves it out");

        run_script(&scratch.0, r#"{"session_id":"../escape","context_window":{"context_window_size":200000,"current_usage":{"input_tokens":1}}}"#);
        assert_eq!(fs::read_dir(claude.join(CONTEXT_DIR)).unwrap().count(), 1, "only a session with a real id is written");
    }

    #[test]
    fn a_connected_script_older_than_this_build_is_replaced_and_other_files_are_not() {
        let scratch = Scratch::new("refresh");
        connect(&scratch.0).unwrap();
        assert_eq!(refresh_script(&scratch.0), Ok(false), "a current script is left alone");
        fs::write(scratch.0.join(SCRIPT_NAME), "# an older copy\n").unwrap();
        assert_eq!(refresh_script(&scratch.0), Ok(true));
        assert_eq!(fs::read_to_string(scratch.0.join(SCRIPT_NAME)).unwrap(), SCRIPT);
        assert!(!scratch.0.join(format!("{SCRIPT_NAME}.tmp")).exists());
    }

    #[test]
    fn a_script_is_not_refreshed_when_the_status_line_is_off_or_not_nesas() {
        let off = Scratch::new("refresh-off");
        assert_eq!(refresh_script(&off.0), Ok(false));
        assert!(!off.0.join(SCRIPT_NAME).exists(), "nothing is written while Nesa is not connected");
        let foreign = Scratch::new("refresh-foreign");
        fs::write(foreign.0.join(SETTINGS_NAME), r#"{"statusLine":{"type":"command","command":"~/.claude/statusline.sh"}}"#).unwrap();
        assert_eq!(refresh_script(&foreign.0), Ok(false));
        assert!(!foreign.0.join(SCRIPT_NAME).exists());
    }
}
