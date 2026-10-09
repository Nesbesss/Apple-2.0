use std::sync::atomic::{AtomicBool, Ordering};

/// Whether finished-session sounds and notifications are on. The UI owns the setting.
static ALERTS: AtomicBool = AtomicBool::new(true);

pub fn set_alerts_enabled(enabled: bool) {
    ALERTS.store(enabled, Ordering::Relaxed);
}

pub fn alerts_enabled() -> bool {
    ALERTS.load(Ordering::Relaxed)
}

/// Plays a short, quiet completion sound. Runs on its own thread so nothing waits on it.
pub fn agent_done() {
    if !alerts_enabled() {
        return;
    }
    std::thread::spawn(|| {
        let _ = std::process::Command::new("afplay")
            .args(["-v", "0.35", "/System/Library/Sounds/Ping.aiff"])
            .status();
    });
}

/// System sounds the UI may ask for by name. Anything else is refused, so the UI never picks a file.
const UI_SOUNDS: &[&str] = &["Tink", "Pop", "Glass", "Ping"];

/// Plays one of `UI_SOUNDS` at a low volume. Runs on its own thread so nothing waits on it.
pub fn play_ui_sound(name: &str) -> Result<(), String> {
    if !UI_SOUNDS.contains(&name) {
        return Err(format!("Unknown sound: {name}"));
    }
    let path = format!("/System/Library/Sounds/{name}.aiff");
    std::thread::spawn(move || {
        let _ = std::process::Command::new("afplay").args(["-v", "0.3", &path]).status();
    });
    Ok(())
}
