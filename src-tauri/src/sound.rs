use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Whether finished-session sounds and notifications are on. The UI owns the setting.
static ALERTS: AtomicBool = AtomicBool::new(true);

/// Index into `DONE_PINGS` of the sound a finished agent plays. The UI owns the setting and sends it on launch.
static DONE_PING: AtomicUsize = AtomicUsize::new(0);

/// Sounds an agent may play when it finishes, by macOS system sound name. The first one is the default.
/// Anything else is refused, so the UI never picks a file.
pub const DONE_PINGS: &[&str] = &[
    "Ping", "Glass", "Hero", "Funk", "Frog", "Bottle", "Blow", "Morse", "Purr", "Submarine", "Sosumi",
];

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
    play_file(DONE_PINGS[DONE_PING.load(Ordering::Relaxed)], "0.35");
}

/// Sets the sound that `agent_done` plays. An unknown name is refused and the stored choice stays as it was.
pub fn set_done_ping(name: &str) -> Result<(), String> {
    DONE_PING.store(ping_index(name)?, Ordering::Relaxed);
    Ok(())
}

/// Plays one of `DONE_PINGS` so the user can hear it in settings. The alerts switch does not apply, since the user asked for it.
pub fn preview_done_ping(name: &str) -> Result<(), String> {
    play_file(DONE_PINGS[ping_index(name)?], "0.35");
    Ok(())
}

/// Position of `name` in `DONE_PINGS`. Only names from that list ever reach `afplay`.
fn ping_index(name: &str) -> Result<usize, String> {
    DONE_PINGS.iter().position(|&ping| ping == name).ok_or_else(|| format!("Unknown ping: {name}"))
}

/// System sounds the UI may ask for by name. Anything else is refused, so the UI never picks a file.
const UI_SOUNDS: &[&str] = &["Tink", "Pop", "Glass"];

/// Plays one of `UI_SOUNDS` at a low volume. Runs on its own thread so nothing waits on it.
pub fn play_ui_sound(name: &str) -> Result<(), String> {
    if !UI_SOUNDS.contains(&name) {
        return Err(format!("Unknown sound: {name}"));
    }
    play_file(name, "0.3");
    Ok(())
}

/// Plays a macOS system sound at the given volume. Runs on its own thread so nothing waits on it.
fn play_file(name: &str, volume: &'static str) {
    let path = format!("/System/Library/Sounds/{name}.aiff");
    std::thread::spawn(move || {
        let _ = std::process::Command::new("afplay").args(["-v", volume, &path]).status();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_done_ping_is_a_system_sound() {
        for ping in DONE_PINGS {
            let path = format!("/System/Library/Sounds/{ping}.aiff");
            assert!(std::path::Path::new(&path).exists(), "missing {path}");
        }
    }

    #[test]
    fn only_listed_pings_can_be_chosen() {
        for (index, ping) in DONE_PINGS.iter().enumerate() {
            set_done_ping(ping).unwrap();
            assert_eq!(DONE_PING.load(Ordering::Relaxed), index);
        }
        assert!(set_done_ping("../../etc/passwd").is_err());
        assert!(preview_done_ping("Tink").is_err());
        assert_eq!(DONE_PING.load(Ordering::Relaxed), DONE_PINGS.len() - 1);
    }
}
