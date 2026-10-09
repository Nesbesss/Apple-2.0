//! Image generation through Codex. Codex's image tool writes PNG files into `generated_images/<session id>/`,
//! and its JSON stream names the session but not the files. A request therefore runs one ephemeral
//! `codex exec`, reads the session id from its stream, and collects the PNGs in that session's folder.

use crate::agents::login_shell;
use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

type CmdResult<T> = Result<T, String>;

/// The Codex feature flag that turns image generation on. `codex features list` reports its state.
const FEATURE: &str = "image_generation";
/// A features listing that takes longer than this counts as no answer, so the Home block stays hidden.
const LISTING_LIMIT: Duration = Duration::from_secs(20);
/// Image generation is slow. A request still running after this is stopped.
const REQUEST_LIMIT: Duration = Duration::from_secs(15 * 60);
/// Longest description sent. More than this is never needed to draw one picture.
const MAX_PROMPT_CHARS: usize = 2000;
/// How long an answer about availability is kept before it is asked again.
const AVAILABILITY_TTL: Duration = Duration::from_secs(60);

/// The process of the request in flight, if any.
static RUNNING: Mutex<Option<u32>> = Mutex::new(None);
/// True while a request holds the slot. One request runs at a time.
static BUSY: AtomicBool = AtomicBool::new(false);
/// Set when the user stopped the request, so it ends as "stopped" and not as a failure.
static CANCELLED: AtomicBool = AtomicBool::new(false);
/// Set when the request ran past `REQUEST_LIMIT` and was stopped.
static TIMED_OUT: AtomicBool = AtomicBool::new(false);
/// The last answer about availability, and when it was given.
static AVAILABLE: Mutex<Option<(Instant, bool)>> = Mutex::new(None);

#[derive(Serialize)]
pub struct ImageGen {
    /// The Codex session the images belong to. Empty when the request was stopped.
    pub thread_id: String,
    /// Absolute paths of the PNG files the request wrote, oldest first.
    pub images: Vec<String>,
    /// True when the user stopped the request. Nothing is reported as a failure then.
    pub stopped: bool,
}

/// What one line of `codex exec --json` tells the request.
#[derive(Debug, PartialEq)]
enum Event {
    Thread(String),
    Message(String),
    Failure(String),
    Other,
}

/// The parts of a request's stream that explain how it ended.
#[derive(Default)]
struct Summary {
    thread_id: Option<String>,
    message: Option<String>,
    failure: Option<String>,
}

impl Summary {
    fn add(&mut self, event: Event) {
        match event {
            Event::Thread(id) => self.thread_id = Some(id),
            Event::Message(text) => self.message = Some(text),
            Event::Failure(text) => self.failure = Some(text),
            Event::Other => {}
        }
    }
}

/// Holds the slot for one request. Dropping it frees the slot and forgets the process, however the request ends.
struct Claim;

impl Claim {
    fn take() -> Option<Claim> {
        (!BUSY.swap(true, Ordering::SeqCst)).then_some(Claim)
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        *RUNNING.lock().unwrap() = None;
        BUSY.store(false, Ordering::SeqCst);
    }
}

/// The folder Codex writes generated images into, one subfolder per session.
pub fn generated_dir() -> PathBuf {
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
        .unwrap_or_default();
    home.join("generated_images")
}

/// Where requests run. It is outside every project, so Codex finds no repository to read.
fn work_dir() -> CmdResult<PathBuf> {
    let dir = dirs::data_dir().unwrap_or_default().join("SessionDeck").join("imagegen");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    Ok(dir)
}

/// `codex` run as a terminal runs it, through the login shell, so PATH and the shell profile apply.
fn codex_command() -> Command {
    let mut command = Command::new(login_shell());
    command.args(["-i", "-l", "-c", r#"exec "$0" "$@""#, "codex"]);
    command
}

/// A line of `codex features list` reads "<name> <stage> <state>", and a stage can hold spaces, so the
/// state is the last word on the line.
pub fn feature_enabled(listing: &str, feature: &str) -> bool {
    listing
        .lines()
        .find_map(|line| {
            let mut words = line.split_whitespace();
            let name = words.next()?;
            let state = words.last()?;
            (name == feature).then(|| state == "true")
        })
        .unwrap_or(false)
}

/// Whether this Codex can make images, as its own feature listing says. A failure, a timeout or a missing
/// `codex` all count as no. The flag does not prove the account may use it, so a request can still fail.
pub fn available() -> bool {
    let cached = *AVAILABLE.lock().unwrap();
    if let Some((at, yes)) = cached {
        if at.elapsed() < AVAILABILITY_TTL {
            return yes;
        }
    }
    let mut command = codex_command();
    command.args(["features", "list"]);
    let yes = output_within(command, LISTING_LIMIT)
        .is_some_and(|out| feature_enabled(&String::from_utf8_lossy(&out.stdout), FEATURE));
    *AVAILABLE.lock().unwrap() = Some((Instant::now(), yes));
    yes
}

/// The message that asks Codex for one picture. The user's words come after the instruction, so they are
/// read as a description rather than as a new instruction.
pub fn request_text(prompt: &str) -> CmdResult<String> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err("Describe the image first".into());
    }
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        return Err(format!("Keep the description under {MAX_PROMPT_CHARS} characters"));
    }
    Ok(format!(
        "Generate exactly one image with your image generation tool, following this description: {prompt}\n\
         Reply with one short sentence and write no files yourself."
    ))
}

/// Arguments for one run: a non-interactive, ephemeral session with no write access, in the work folder.
/// `notify=[]` keeps the user's notify hook quiet for these background runs. The rest of the config is used.
pub fn exec_args(request: &str, work: &Path) -> Vec<String> {
    vec![
        "exec".into(),
        "--json".into(),
        "--ephemeral".into(),
        "--skip-git-repo-check".into(),
        "-s".into(),
        "read-only".into(),
        "-c".into(),
        "notify=[]".into(),
        "-C".into(),
        work.display().to_string(),
        "--".into(),
        request.into(),
    ]
}

/// Reads one line of the stream. Lines that are not JSON, and events the request does not need, are ignored.
fn read_event(line: &str) -> Event {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return Event::Other;
    };
    match value["type"].as_str() {
        Some("thread.started") => value["thread_id"].as_str().map_or(Event::Other, |id| Event::Thread(id.into())),
        Some("item.completed") if value["item"]["type"] == "agent_message" => value["item"]["text"]
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .map_or(Event::Other, |text| Event::Message(text.into())),
        Some(kind) if kind.contains("error") || kind.contains("failed") => {
            let text = value["message"].as_str().or(value["error"]["message"].as_str()).unwrap_or(kind);
            Event::Failure(text.into())
        }
        _ => Event::Other,
    }
}

/// PNG files in `dir` written at or after `since`, oldest first.
fn images_since(dir: &Path, since: SystemTime) -> Vec<PathBuf> {
    let mut found: Vec<(SystemTime, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let is_png = path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("png"));
            let modified = entry.metadata().ok()?.modified().ok()?;
            (is_png && modified >= since).then_some((modified, path))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, path)| path).collect()
}

/// Why a request ended without an image: Codex's own failure first, then its last message, then stderr.
fn no_image_reason(summary: &Summary, clean_exit: bool, stderr: &str) -> String {
    let detail = summary
        .failure
        .clone()
        .or_else(|| summary.message.clone())
        .or_else(|| stderr.lines().rev().find(|line| !line.trim().is_empty()).map(str::to_string))
        .unwrap_or_else(|| "Codex returned nothing.".into());
    if summary.thread_id.is_none() {
        format!("Codex did not start. {detail}")
    } else if clean_exit {
        format!("Codex finished without an image. {detail}")
    } else {
        format!("Codex could not make the image. {detail}")
    }
}

/// Runs a request and returns the images it wrote. This blocks for as long as Codex takes, so it runs off the
/// UI thread. A second request while one runs is refused.
pub fn generate(prompt: &str) -> CmdResult<ImageGen> {
    let request = request_text(prompt)?;
    let _claim = Claim::take().ok_or("An image is already being made")?;
    let work = work_dir()?;
    CANCELLED.store(false, Ordering::SeqCst);
    TIMED_OUT.store(false, Ordering::SeqCst);

    let started = SystemTime::now();
    let mut command = codex_command();
    command
        .args(exec_args(&request, &work))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| format!("Could not start Codex: {e}"))?;
    let pid = child.id();
    *RUNNING.lock().unwrap() = Some(pid);
    watch(pid);

    let stderr = child.stderr.take().map(|pipe| thread::spawn(move || read_all(pipe)));
    let mut summary = Summary::default();
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            summary.add(read_event(&line));
        }
    }
    let status = child.wait().map_err(|e| format!("Codex stopped unexpectedly: {e}"))?;
    let stderr_text = stderr.and_then(|reader| reader.join().ok()).unwrap_or_default();

    if CANCELLED.load(Ordering::SeqCst) {
        return Ok(ImageGen { thread_id: String::new(), images: Vec::new(), stopped: true });
    }
    if TIMED_OUT.load(Ordering::SeqCst) {
        return Err("The request took too long and was stopped".into());
    }
    let images = match &summary.thread_id {
        // A little slack on the start time, so a clock that is slightly behind does not hide the first file.
        Some(id) => images_since(&generated_dir().join(id), started - Duration::from_secs(1)),
        None => Vec::new(),
    };
    if images.is_empty() {
        return Err(no_image_reason(&summary, status.success(), &stderr_text));
    }
    Ok(ImageGen {
        thread_id: summary.thread_id.unwrap_or_default(),
        images: images.iter().map(|path| path.display().to_string()).collect(),
        stopped: false,
    })
}

/// Stops the request in flight, if there is one. Its own call then returns "stopped".
pub fn cancel() {
    let pid = *RUNNING.lock().unwrap();
    if let Some(pid) = pid {
        CANCELLED.store(true, Ordering::SeqCst);
        kill(pid);
    }
}

/// Stops a request that runs past `REQUEST_LIMIT`. Does nothing when the request has already ended.
fn watch(pid: u32) {
    thread::spawn(move || {
        thread::sleep(REQUEST_LIMIT);
        if *RUNNING.lock().unwrap() == Some(pid) {
            TIMED_OUT.store(true, Ordering::SeqCst);
            kill(pid);
        }
    });
}

fn kill(pid: u32) {
    let _ = Command::new("/bin/kill")
        .args(["-TERM", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn read_all(mut pipe: impl Read) -> String {
    let mut bytes = Vec::new();
    let _ = pipe.read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Runs a command and returns its output, or None when it fails or runs past `limit`.
fn output_within(mut command: Command, limit: Duration) -> Option<Output> {
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let pid = child.id();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(child.wait_with_output());
    });
    match receiver.recv_timeout(limit) {
        Ok(Ok(output)) if output.status.success() => Some(output),
        Ok(_) => None,
        Err(_) => {
            kill(pid);
            None
        }
    }
}

/// Puts a generated PNG or JPEG on the clipboard as an image, so a chat takes it with a paste.
pub fn copy_to_clipboard(path: &Path) -> CmdResult<()> {
    let class = match path.extension().and_then(|ext| ext.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "PNGf",
        Some("jpg" | "jpeg") => "JPEG",
        _ => return Err("Only PNG and JPEG images can be copied".into()),
    };
    if !path.is_file() {
        return Err("That image is no longer there".into());
    }
    // The path is an argument, never part of the script, so no file name can change what runs.
    let script = format!("set the clipboard to (read (POSIX file (item 1 of argv)) as «class {class}»)");
    let status = Command::new("osascript")
        .args(["-e", "on run argv", "-e", &script, "-e", "end run"])
        .arg(path)
        .status()
        .map_err(|e| format!("Could not copy: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Could not copy the image".into())
    }
}

/// Shows a generated image in Finder. Only files inside Codex's generated folder can be shown.
pub fn reveal(path: &Path) -> CmdResult<()> {
    let root = fs::canonicalize(generated_dir()).map_err(|_| "No images have been made yet".to_string())?;
    let file = fs::canonicalize(path).map_err(|_| "That image is no longer there".to_string())?;
    if !file.starts_with(&root) {
        return Err("Only images from Codex's generated folder can be shown".into());
    }
    let status = Command::new("open")
        .arg("-R")
        .arg(&file)
        .status()
        .map_err(|e| format!("Could not open Finder: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Finder could not show that image".into())
    }
}

/// Whether this Codex can make images. The Home block shows only when it can.
#[tauri::command]
pub async fn imagegen_available() -> bool {
    tauri::async_runtime::spawn_blocking(available).await.unwrap_or(false)
}

/// Makes images from a description. It runs for as long as Codex takes, off the UI thread.
#[tauri::command]
pub async fn imagegen_generate(prompt: String) -> CmdResult<ImageGen> {
    tauri::async_runtime::spawn_blocking(move || generate(&prompt))
        .await
        .map_err(|e| e.to_string())?
}

/// Stops the request in flight.
#[tauri::command]
pub fn imagegen_cancel() {
    cancel();
}

#[tauri::command]
pub async fn imagegen_copy(path: String) -> CmdResult<()> {
    tauri::async_runtime::spawn_blocking(move || copy_to_clipboard(Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn imagegen_reveal(path: String) -> CmdResult<()> {
    tauri::async_runtime::spawn_blocking(move || reveal(Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use uuid::Uuid;

    fn has_pair(args: &[String], first: &str, second: &str) -> bool {
        args.windows(2).any(|pair| pair[0] == first && pair[1] == second)
    }

    /// A fresh folder under the system temp dir, removed when the test ends.
    struct Folder(PathBuf);

    impl Folder {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("nesa-imagegen-{}", Uuid::new_v4()));
            fs::create_dir_all(&dir).unwrap();
            Folder(dir)
        }
        fn file(&self, name: &str, mtime_secs: u64) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, b"png").unwrap();
            let time = SystemTime::UNIX_EPOCH + Duration::from_secs(mtime_secs);
            fs::File::options().write(true).open(&path).unwrap().set_modified(time).unwrap();
            path
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_feature_state_is_the_last_word_of_its_line() {
        let listing = "image_generation                         stable             true\n\
                       code_mode                                under development  false\n\
                       memories                                 stable             false\n";
        assert!(feature_enabled(listing, FEATURE));
        assert!(!feature_enabled(listing, "memories"));
        assert!(!feature_enabled(listing, "code_mode"));
        assert!(!feature_enabled(listing, "missing"));
        assert!(!feature_enabled("", FEATURE));
    }

    #[test]
    fn a_run_is_read_only_ephemeral_quiet_and_takes_the_request_last() {
        let args = exec_args("a red circle", Path::new("/tmp/Application Support/work"));
        assert!(has_pair(&args, "-s", "read-only"));
        assert!(args.contains(&"--ephemeral".to_string()));
        assert!(has_pair(&args, "-c", "notify=[]"));
        assert!(has_pair(&args, "-C", "/tmp/Application Support/work"));
        assert_eq!(args[args.len() - 2], "--");
        assert_eq!(args.last().map(String::as_str), Some("a red circle"));
    }

    #[test]
    fn a_request_must_say_something_and_stay_short() {
        assert!(request_text("   ").is_err());
        assert!(request_text(&"x".repeat(MAX_PROMPT_CHARS + 1)).is_err());
        let text = request_text("  a fox in snow  ").unwrap();
        assert!(text.contains("following this description: a fox in snow\n"));
        assert!(text.contains("exactly one image"));
    }

    #[test]
    fn events_give_the_session_the_message_and_any_failure() {
        assert_eq!(read_event(r#"{"type":"thread.started","thread_id":"abc"}"#), Event::Thread("abc".into()));
        assert_eq!(
            read_event(r#"{"type":"item.completed","item":{"type":"agent_message","text":"Done."}}"#),
            Event::Message("Done.".into())
        );
        assert_eq!(
            read_event(r#"{"type":"item.completed","item":{"type":"agent_message","text":"  "}}"#),
            Event::Other
        );
        assert_eq!(
            read_event(r#"{"type":"turn.failed","error":{"message":"usage limit reached"}}"#),
            Event::Failure("usage limit reached".into())
        );
        assert_eq!(read_event(r#"{"type":"turn.completed"}"#), Event::Other);
        assert_eq!(read_event("Reading additional input from stdin..."), Event::Other);
    }

    #[test]
    fn the_images_are_the_pngs_written_since_the_marker() {
        let folder = Folder::new();
        folder.file("new.png", 2_000);
        folder.file("older.png", 500);
        folder.file("notes.txt", 2_000);
        folder.file("upper.PNG", 3_000);
        let found: Vec<String> = images_since(&folder.0, SystemTime::UNIX_EPOCH + Duration::from_secs(1_000))
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(found, vec!["new.png", "upper.PNG"]);
    }

    #[test]
    fn a_failed_request_explains_itself_from_the_best_source() {
        let mut summary = Summary::default();
        summary.add(Event::Thread("t".into()));
        summary.add(Event::Message("Here is nothing.".into()));
        assert_eq!(
            no_image_reason(&summary, true, "noise\n"),
            "Codex finished without an image. Here is nothing."
        );
        summary.add(Event::Failure("usage limit reached".into()));
        assert_eq!(
            no_image_reason(&summary, false, ""),
            "Codex could not make the image. usage limit reached"
        );
        assert_eq!(
            no_image_reason(&Summary::default(), false, "login failed\n"),
            "Codex did not start. login failed"
        );
    }

    #[test]
    fn only_pngs_and_jpegs_are_copied_and_only_generated_images_are_shown() {
        assert!(copy_to_clipboard(Path::new("/tmp/picture.gif")).is_err());
        assert!(copy_to_clipboard(Path::new("/definitely/not/here.png")).is_err());
        assert!(reveal(Path::new("/etc/hosts")).is_err());
    }

    #[test]
    fn a_missing_feature_listing_counts_as_unavailable() {
        let folder = Folder::new();
        let script = folder.0.join("codex");
        fs::write(&script, "#!/bin/sh\nexit 3\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let mut command = Command::new(&script);
        command.args(["features", "list"]);
        assert!(output_within(command, Duration::from_secs(5)).is_none());
    }

    /// Copies a real generated PNG and reads the clipboard back. It replaces what is on the clipboard, so it
    /// only runs with `--ignored`.
    #[test]
    #[ignore]
    fn live_copy_puts_a_picture_on_the_clipboard() {
        let picture = fs::read_dir(generated_dir())
            .expect("a generated folder exists")
            .flatten()
            .flat_map(|session| fs::read_dir(session.path()).into_iter().flatten().flatten())
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|ext| ext == "png"))
            .expect("at least one generated PNG");
        copy_to_clipboard(&picture).expect("the copy succeeds");
        let info = Command::new("osascript").args(["-e", "clipboard info"]).output().expect("osascript runs");
        let info = String::from_utf8_lossy(&info.stdout).into_owned();
        assert!(info.contains("PNGf") || info.contains("class PNGf"), "clipboard info: {info}");
        println!("CLIPBOARD {info}");
    }

    /// Asks the real `codex` through the login shell. It only lists features, so it costs nothing.
    #[test]
    #[ignore]
    fn live_availability() {
        assert!(available(), "image generation should be on for this Codex");
    }

    /// Calls the real `codex` on this machine. It costs one image, so it only runs with `--ignored`.
    #[test]
    #[ignore]
    fn live_availability_and_one_generation() {
        assert!(available(), "image generation should be on for this Codex");
        let result = generate("a plain blue square, nothing else").expect("the request succeeds");
        assert!(!result.stopped);
        assert!(!result.images.is_empty());
        assert!(Path::new(&result.images[0]).is_file());
        println!("LIVE thread={} images={}", result.thread_id, result.images.len());
    }
}
