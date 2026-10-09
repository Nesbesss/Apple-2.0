use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use uuid::Uuid;

/// Whether finished-session sounds and notifications are on. The UI owns the setting.
static ALERTS: AtomicBool = AtomicBool::new(true);

/// Index into `DONE_PINGS` of the sound a finished agent plays. The UI owns the setting and sends it on launch.
static DONE_PING: AtomicUsize = AtomicUsize::new(0);

/// A sound an agent plays when it finishes. The WAV files are made by `scripts/make-done-pings.mjs`.
struct Ping {
    name: &'static str,
    wav: &'static [u8],
}

/// Sounds an agent may play when it finishes. The first one is the default. Anything else is refused,
/// so the UI never picks a file.
const DONE_PINGS: &[Ping] = &[
    Ping { name: "Chime", wav: include_bytes!("../assets/sounds/chime.wav") },
    Ping { name: "Marimba", wav: include_bytes!("../assets/sounds/marimba.wav") },
    Ping { name: "Harp", wav: include_bytes!("../assets/sounds/harp.wav") },
    Ping { name: "Droplet", wav: include_bytes!("../assets/sounds/droplet.wav") },
    Ping { name: "Glow", wav: include_bytes!("../assets/sounds/glow.wav") },
    Ping { name: "Lift", wav: include_bytes!("../assets/sounds/lift.wav") },
    Ping { name: "Knock", wav: include_bytes!("../assets/sounds/knock.wav") },
    Ping { name: "Twinkle", wav: include_bytes!("../assets/sounds/twinkle.wav") },
    Ping { name: "Pluck", wav: include_bytes!("../assets/sounds/pluck.wav") },
    Ping { name: "Echo", wav: include_bytes!("../assets/sounds/echo.wav") },
    Ping { name: "Ripple", wav: include_bytes!("../assets/sounds/ripple.wav") },
];

pub fn set_alerts_enabled(enabled: bool) {
    ALERTS.store(enabled, Ordering::Relaxed);
}

pub fn alerts_enabled() -> bool {
    ALERTS.load(Ordering::Relaxed)
}

/// Plays the chosen done ping quietly. The file is written here; the sound plays on its own thread.
pub fn agent_done() {
    if !alerts_enabled() {
        return;
    }
    let _ = play_done_ping(&DONE_PINGS[DONE_PING.load(Ordering::Relaxed)], "0.35");
}

/// Sets the sound that `agent_done` plays. An unknown name is refused and the stored choice stays as it was.
pub fn set_done_ping(name: &str) -> Result<(), String> {
    DONE_PING.store(ping_index(name)?, Ordering::Relaxed);
    Ok(())
}

/// Plays one of `DONE_PINGS` so the user can hear it in settings. The alerts switch does not apply, since the user asked for it.
pub fn preview_done_ping(name: &str) -> Result<(), String> {
    play_done_ping(&DONE_PINGS[ping_index(name)?], "0.35")
}

/// Position of `name` in `DONE_PINGS`. Only names from that list ever reach `afplay`.
fn ping_index(name: &str) -> Result<usize, String> {
    DONE_PINGS.iter().position(|ping| ping.name == name).ok_or_else(|| format!("Unknown ping: {name}"))
}

fn play_done_ping(ping: &Ping, volume: &'static str) -> Result<(), String> {
    afplay(ping_file(ping)?, volume);
    Ok(())
}

/// The file a done ping plays from. Its name holds a hash of the sound, so a redesigned sound is written
/// under a new name and never plays from an older copy. The file is written under a unique temporary name
/// and renamed into place, so nothing reads a half-written file.
fn ping_file(ping: &Ping) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("nesa-sounds");
    let mut hasher = DefaultHasher::new();
    ping.wav.hash(&mut hasher);
    let path = dir.join(format!("{}-{:016x}.wav", ping.name.to_lowercase(), hasher.finish()));
    // The temp folder can be cleared at any time, so check on every play.
    if !path.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("Could not make {}: {e}", dir.display()))?;
        let temp = dir.join(format!("write-{}.tmp", Uuid::new_v4()));
        fs::write(&temp, ping.wav).map_err(|e| format!("Could not write {}: {e}", temp.display()))?;
        fs::rename(&temp, &path).map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    }
    Ok(path)
}

/// System sounds the UI may ask for by name. Anything else is refused, so the UI never picks a file.
const UI_SOUNDS: &[&str] = &["Tink", "Pop", "Glass"];

/// Plays one of `UI_SOUNDS` at a low volume. Runs on its own thread so nothing waits on it.
pub fn play_ui_sound(name: &str) -> Result<(), String> {
    if !UI_SOUNDS.contains(&name) {
        return Err(format!("Unknown sound: {name}"));
    }
    afplay(PathBuf::from(format!("/System/Library/Sounds/{name}.aiff")), "0.3");
    Ok(())
}

/// Plays an audio file with `afplay` at the given volume. Runs on its own thread so nothing waits on it.
fn afplay(path: PathBuf, volume: &'static str) {
    std::thread::spawn(move || {
        let _ = std::process::Command::new("afplay").args(["-v", volume]).arg(&path).status();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_done_ping_is_a_wav_file() {
        for ping in DONE_PINGS {
            assert_eq!(&ping.wav[0..4], b"RIFF", "{}", ping.name);
            assert_eq!(&ping.wav[8..12], b"WAVE", "{}", ping.name);
        }
    }

    #[test]
    fn a_written_ping_reads_back_unchanged() {
        let path = ping_file(&DONE_PINGS[0]).unwrap();
        assert_eq!(fs::read(&path).unwrap(), DONE_PINGS[0].wav);
    }

    #[test]
    fn only_listed_pings_can_be_chosen() {
        for (index, ping) in DONE_PINGS.iter().enumerate() {
            set_done_ping(ping.name).unwrap();
            assert_eq!(DONE_PING.load(Ordering::Relaxed), index);
        }
        assert!(set_done_ping("../../etc/passwd").is_err());
        assert!(preview_done_ping("Tink").is_err());
        assert_eq!(DONE_PING.load(Ordering::Relaxed), DONE_PINGS.len() - 1);
    }
}
