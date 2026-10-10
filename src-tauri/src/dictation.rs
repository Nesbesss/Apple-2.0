//! Dictation: a Swift sidecar (`speech-helper`) listens while the user holds the voice-mode keys.
//! The helper runs as its own process, so a speech failure can never take down a terminal
//! session. Its JSON lines are forwarded to the UI as `dictation` events.
//!
//! The helper also watches the modifier keys in every app (an event tap that only reads flags and
//! whether a key or click came in, never which key), so the hold works outside Nesa. The chord
//! state machine below turns those reports into hold, cancel and release events.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::Mutex,
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{corrections, AppState};

type CmdResult<T> = Result<T, String>;

/// Most vocabulary words the engine is given for one hold. More than this bias it too far.
const MAX_WORDS: usize = 100;
/// Longest word or phrase the engine is given.
const MAX_WORD_CHARS: usize = 80;
/// How long the chord must be held alone before dictation starts. A quick tap does nothing.
const HOLD_DELAY: Duration = Duration::from_millis(180);

/// The dictation pill over other apps. It is transparent and never takes focus or clicks.
const HUD_WIDTH: f64 = 640.0;
const HUD_HEIGHT: f64 = 112.0;
/// Where the top of the window sits, below the menu bar. The pill is centred in the window.
const HUD_TOP: f64 = 24.0;

const ALT: u8 = 1 << 0;
const CTRL: u8 = 1 << 1;
const META: u8 = 1 << 2;
const SHIFT: u8 = 1 << 3;

fn modifier_bit(name: &str) -> u8 {
    match name {
        "alt" => ALT,
        "ctrl" => CTRL,
        "meta" => META,
        "shift" => SHIFT,
        _ => 0,
    }
}

/// The modifier keys the helper reports as down, as bits.
fn flag_bits(event: &Value) -> u8 {
    ["alt", "ctrl", "meta", "shift"]
        .into_iter()
        .filter(|name| event.get(*name).and_then(Value::as_bool).unwrap_or(false))
        .fold(0, |bits, name| bits | modifier_bit(name))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    /// The chord is down alone and the hold delay is running.
    Pending,
    /// The hold delay passed, so dictation started.
    Held,
    /// A key or a click came during the hold. The chord still has to be let go.
    Ignoring,
}

/// What the chord means for the app. `on_step` turns each one into work or an event.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    Nothing,
    /// The chord went down alone. After `HOLD_DELAY`, call `Chord::held` with this generation.
    Wait(u64),
    /// The chord was held for the delay: dictation starts.
    Held,
    /// A key or a click came during the hold, so the hold is cancelled.
    Cancelled,
    /// The chord was let go.
    Released,
}

/// The voice-mode chord, followed across every app. Pure state, so the rules are unit-tested.
#[derive(Default)]
pub struct Chord {
    keys: u8,
    phase: Phase,
    /// Bumped whenever a pending delay stops being valid, so a stale timer does nothing.
    generation: u64,
}

impl Chord {
    fn set_keys(&mut self, keys: u8) {
        self.keys = keys;
        self.phase = Phase::Idle;
        self.generation += 1;
    }

    /// The modifier keys that are down right now.
    fn modifiers(&mut self, pressed: u8) -> Step {
        if self.keys == 0 {
            return Step::Nothing;
        }
        if self.phase == Phase::Idle {
            // Only the chord alone starts a hold. Option with Shift is a shortcut, not voice.
            if pressed == self.keys {
                self.phase = Phase::Pending;
                self.generation += 1;
                return Step::Wait(self.generation);
            }
            return Step::Nothing;
        }
        // Extra keys do not end a hold, the same as before. Letting go of a chord key does.
        if pressed & self.keys == self.keys {
            return Step::Nothing;
        }
        let was = self.phase;
        self.phase = Phase::Idle;
        self.generation += 1;
        match was {
            Phase::Held | Phase::Ignoring => Step::Released,
            _ => Step::Nothing,
        }
    }

    /// A key or a click that is not part of the chord.
    fn input(&mut self) -> Step {
        match self.phase {
            Phase::Pending => {
                self.phase = Phase::Ignoring;
                self.generation += 1;
                Step::Nothing
            }
            Phase::Held => {
                self.phase = Phase::Ignoring;
                Step::Cancelled
            }
            Phase::Idle | Phase::Ignoring => Step::Nothing,
        }
    }

    /// The hold delay ran out for `generation`. Only the timer that is still current counts.
    fn held(&mut self, generation: u64) -> Step {
        if self.phase == Phase::Pending && generation == self.generation {
            self.phase = Phase::Held;
            Step::Held
        } else {
            Step::Nothing
        }
    }
}

struct Helper {
    child: Child,
    stdin: ChildStdin,
}

/// The running helper, started on first use and restarted if it exits.
#[derive(Default)]
pub struct Dictation {
    helper: Mutex<Option<Helper>>,
    pub chord: Mutex<Chord>,
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
            let Ok(event) = serde_json::from_str::<Value>(&line) else { continue };
            match event.get("event").and_then(Value::as_str) {
                // Keys only ever reach the chord. The UI never sees them.
                Some("flags") => {
                    let step = app.state::<AppState>().dictation.chord.lock().unwrap().modifiers(flag_bits(&event));
                    on_step(&app, step);
                }
                Some("input") => {
                    let step = app.state::<AppState>().dictation.chord.lock().unwrap().input();
                    on_step(&app, step);
                }
                // The helper saw a correction in another app's text field. Only the fixes go on, never the text.
                Some("edit") => {
                    let inserted = event.get("inserted").and_then(Value::as_str).unwrap_or_default();
                    let edited = event.get("edited").and_then(Value::as_str).unwrap_or_default();
                    let check = corrections::check(inserted, edited);
                    if check.corrections.is_empty() {
                        // Say why nothing was learned, so an edit that was seen is not silently dropped.
                        let _ = app.emit("dictation", json!({ "event": "checked", "status": check.status }));
                    } else {
                        let _ = app.emit("dictation", json!({ "event": "learned", "pairs": check.corrections }));
                    }
                }
                _ => {
                    let _ = app.emit("dictation", event);
                }
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

/// Acts on what the chord decided. A hold that is waiting gets its own timer thread.
fn on_step(app: &AppHandle, step: Step) {
    match step {
        Step::Nothing => {}
        Step::Wait(generation) => {
            let app = app.clone();
            thread::spawn(move || {
                thread::sleep(HOLD_DELAY);
                let step = app.state::<AppState>().dictation.chord.lock().unwrap().held(generation);
                on_step(&app, step);
            });
        }
        Step::Held => {
            // The UI hears "held" first, so it is already recording when the helper names the app.
            emit_keys(app, "held");
            // Note the app the words will go to now, while it is still the one the user is in.
            let state = app.state::<AppState>();
            let _ = send(app, &state, json!({ "cmd": "target" }));
        }
        Step::Cancelled => emit_keys(app, "other"),
        Step::Released => emit_keys(app, "up"),
    }
}

fn emit_keys(app: &AppHandle, state: &str) {
    let _ = app.emit("dictation", json!({ "event": "keys", "state": state }));
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
/// words go in too. `engine` is "parakeet" or "apple". All of it travels with every start, so a helper
/// that was restarted still gets it.
#[tauri::command]
pub fn dictation_start(app: AppHandle, state: State<'_, AppState>, words: Vec<String>, builtin: bool, engine: String) -> CmdResult<()> {
    send(&app, state.inner(), json!({ "cmd": "start", "words": clean_words(words), "builtin": builtin, "engine": engine }))
}

#[tauri::command]
pub fn dictation_stop(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    send(&app, state.inner(), json!({ "cmd": "stop" }))
}

#[tauri::command]
pub fn dictation_cancel(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    send(&app, state.inner(), json!({ "cmd": "cancel" }))
}

/// Sets the voice-mode chord from its modifier names, such as `["alt", "meta"]`.
#[tauri::command]
pub fn dictation_keys(state: State<'_, AppState>, keys: Vec<String>) {
    let bits = keys.iter().fold(0, |bits, name| bits | modifier_bit(name));
    state.dictation.chord.lock().unwrap().set_keys(bits);
}

/// Starts the key watch if it is not running yet, and reports the permissions. Called at launch
/// and whenever Nesa gains focus, so a permission granted in System Settings is picked up.
#[tauri::command]
pub fn dictation_watch(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    send(&app, state.inner(), json!({ "cmd": "watch" }))
}

/// Asks macOS for the two permissions and reports where they stand.
#[tauri::command]
pub fn dictation_request_access(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    send(&app, state.inner(), json!({ "cmd": "request_access" }))
}

/// Types the words into the app that was frontmost when the hold started, by pasting them.
#[tauri::command]
pub fn dictation_paste(app: AppHandle, state: State<'_, AppState>, text: String, press_return: bool, learn: bool) -> CmdResult<()> {
    send(&app, state.inner(), json!({ "cmd": "paste", "text": text, "return": press_return, "learn": learn }))
}

/// The System Settings pane for one of the two permissions.
#[tauri::command]
pub fn dictation_open_privacy(pane: String) -> CmdResult<()> {
    let url = match pane.as_str() {
        "input" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
        "accessibility" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        _ => return Err("Unknown privacy pane".into()),
    };
    Command::new("open").arg(url).status().map_err(|e| format!("Could not open System Settings: {e}"))?;
    Ok(())
}

/// What the pill over other apps shows. Nesa sends it whenever the pill changes. The level bars
/// come from the helper, which the HUD window hears directly.
#[derive(Clone, Serialize, Deserialize)]
pub struct HudState {
    pub visible: bool,
    pub mode: String,
    pub warn: bool,
    pub label: String,
    pub meta: String,
    pub words: String,
    pub kind: String,
}

/// The pill window. It floats above other apps, takes no focus and passes clicks through.
pub fn create_hud(app: &AppHandle) -> CmdResult<()> {
    let hud = WebviewWindowBuilder::new(app, "hud", WebviewUrl::App("hud.html".into()))
        .title("Nesa dictation")
        .inner_size(HUD_WIDTH, HUD_HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focusable(false)
        .resizable(false)
        .visible(false)
        .build()
        .map_err(|e| format!("Could not create the dictation window: {e}"))?;
    hud.set_ignore_cursor_events(true).map_err(|e| e.to_string())
}

/// Centres the pill window at the top of the main screen.
fn place_hud(app: &AppHandle, hud: &WebviewWindow) -> CmdResult<()> {
    let monitor = app
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("There is no screen to show dictation on")?;
    let scale = monitor.scale_factor();
    let origin = monitor.position().to_logical::<f64>(scale);
    let size = monitor.size().to_logical::<f64>(scale);
    let x = origin.x + (size.width - HUD_WIDTH) / 2.0;
    hud.set_position(LogicalPosition::new(x, origin.y + HUD_TOP)).map_err(|e| e.to_string())
}

/// Shows or hides the pill over other apps, and sends it what to draw.
#[tauri::command]
pub fn dictation_hud(app: AppHandle, state: HudState) -> CmdResult<()> {
    let hud = app.get_webview_window("hud").ok_or("The dictation window is missing")?;
    hud.emit("hud", &state).map_err(|e| e.to_string())?;
    if state.visible {
        if !hud.is_visible().unwrap_or(false) {
            place_hud(&app, &hud)?;
        }
        hud.show().map_err(|e| e.to_string())
    } else {
        hud.hide().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPTION: u8 = ALT;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|word| word.to_string()).collect()
    }

    /// A chord that starts on Option alone, and its delay timer generation.
    fn option_chord() -> (Chord, u64) {
        let mut chord = Chord::default();
        chord.set_keys(OPTION);
        let Step::Wait(generation) = chord.modifiers(OPTION) else { panic!("Option alone should start a hold") };
        (chord, generation)
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

    #[test]
    fn a_held_chord_starts_then_releases() {
        let (mut chord, generation) = option_chord();
        assert_eq!(chord.held(generation), Step::Held);
        assert_eq!(chord.modifiers(0), Step::Released);
    }

    #[test]
    fn a_quick_tap_does_nothing() {
        let (mut chord, generation) = option_chord();
        assert_eq!(chord.modifiers(0), Step::Nothing);
        assert_eq!(chord.held(generation), Step::Nothing);
    }

    #[test]
    fn a_key_during_the_delay_cancels_the_hold() {
        let (mut chord, generation) = option_chord();
        assert_eq!(chord.input(), Step::Nothing);
        assert_eq!(chord.held(generation), Step::Nothing);
        // Letting go still reports a release. Nothing started, so the UI has nothing to end.
        assert_eq!(chord.modifiers(0), Step::Released);
    }

    #[test]
    fn a_key_during_a_hold_cancels_once_then_release_ends_it() {
        let (mut chord, generation) = option_chord();
        chord.held(generation);
        assert_eq!(chord.input(), Step::Cancelled);
        assert_eq!(chord.input(), Step::Nothing, "a second key does not cancel twice");
        assert_eq!(chord.modifiers(0), Step::Released);
    }

    #[test]
    fn a_stale_timer_does_nothing() {
        let (mut chord, generation) = option_chord();
        chord.modifiers(0);
        let Step::Wait(next) = chord.modifiers(OPTION) else { panic!("a new press starts a new delay") };
        assert_ne!(generation, next);
        assert_eq!(chord.held(generation), Step::Nothing, "the old delay must not start a hold");
        assert_eq!(chord.held(next), Step::Held);
    }

    #[test]
    fn option_with_shift_is_not_the_chord() {
        let mut chord = Chord::default();
        chord.set_keys(OPTION);
        assert_eq!(chord.modifiers(OPTION | SHIFT), Step::Nothing);
    }

    #[test]
    fn extra_keys_do_not_end_a_hold() {
        let (mut chord, generation) = option_chord();
        chord.held(generation);
        assert_eq!(chord.modifiers(OPTION | SHIFT), Step::Nothing);
        assert_eq!(chord.modifiers(0), Step::Released);
    }

    #[test]
    fn a_two_key_chord_needs_both_keys() {
        let mut chord = Chord::default();
        chord.set_keys(ALT | META);
        assert_eq!(chord.modifiers(ALT), Step::Nothing);
        assert!(matches!(chord.modifiers(ALT | META), Step::Wait(_)));
    }

    #[test]
    fn no_chord_never_fires() {
        let mut chord = Chord::default();
        assert_eq!(chord.modifiers(OPTION), Step::Nothing);
    }

    #[test]
    fn flags_from_the_helper_become_bits() {
        let event = json!({ "event": "flags", "alt": true, "ctrl": false, "meta": true, "shift": false });
        assert_eq!(flag_bits(&event), ALT | META);
        assert_eq!(flag_bits(&json!({ "event": "flags" })), 0);
    }
}
