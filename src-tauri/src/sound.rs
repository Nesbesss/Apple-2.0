/// Plays a short, quiet completion sound. Runs on its own thread so nothing waits on it.
pub fn agent_done() {
    std::thread::spawn(|| {
        let _ = std::process::Command::new("afplay")
            .args(["-v", "0.35", "/System/Library/Sounds/Ping.aiff"])
            .status();
    });
}
