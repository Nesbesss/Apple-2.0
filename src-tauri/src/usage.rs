//! What Claude and Codex have used on this Mac, for the usage bar.
//!
//! Claude: tokens from assistant messages in every transcript under `~/.claude/projects`, over the last
//! five hours. Claude writes one message as several lines while it streams, so each message id counts
//! once, at the largest value seen for each field. Cache reads are kept apart. They are reported, but
//! they are not part of the estimate. Claude Code does not record its plan limit on disk, so this is
//! only an estimate, and the frontend turns it into a percentage only once the user has set a limit.
//!
//! Codex: the rate-limit windows that Codex reports in its own session logs. Those numbers are real.
//!
//! Transcripts can run to hundreds of megabytes, so they are read incrementally. Each file keeps the
//! byte offset it was read to, and a line is only parsed once its newline has arrived.

use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

const HOUR_MS: u64 = 60 * 60 * 1000;
/// Claude's limit window, as the estimate treats it. Claude's real window is block-based, not rolling.
const WINDOW_MS: u64 = 5 * HOUR_MS;
/// Codex rollouts older than this cannot hold a window that is still open. The weekly window is the longest.
const CODEX_LOOKBACK_MS: u64 = 8 * 24 * HOUR_MS;
/// Rollouts read for the newest limit report. Codex writes one per turn, so the newest few are enough.
const CODEX_FILES: usize = 6;
/// Bytes read from the end of each rollout. A single report is a few hundred bytes.
const CODEX_TAIL_BYTES: u64 = 256 * 1024;
/// Transcripts are searched this many folders deep: projects/<folder>/<session>/subagents/<file>.
const CLAUDE_DEPTH: usize = 5;
/// Cache writes cost 1.25 times input, and output costs five times input, in every current model's price.
const CACHE_WRITE_RATIO: f64 = 1.25;
const OUTPUT_RATIO: f64 = 5.0;

/// Shared by every call to `usage_snapshot`, so the transcripts are only read incrementally once.
#[derive(Default)]
pub struct UsageState(pub Arc<Mutex<Meter>>);

#[derive(Default)]
pub struct Meter {
    files: HashMap<PathBuf, Cursor>,
    messages: HashMap<String, Message>,
}

/// Where a transcript was read to, and the unfinished line after that point.
#[derive(Default)]
pub struct Cursor {
    offset: u64,
    /// The unfinished last line of the file, kept until its newline arrives.
    partial: Vec<u8>,
}

/// The complete lines appended to `path` since `cursor` last read it, which moves the cursor past them. A file shorter
/// than the cursor was replaced, so it is read again from the top. The unfinished last line waits for its newline.
pub fn read_appended(path: &Path, cursor: &mut Cursor) -> Vec<String> {
    let Ok(mut file) = File::open(path) else { return Vec::new() };
    let Ok(len) = file.metadata().map(|m| m.len()) else { return Vec::new() };
    if len < cursor.offset {
        *cursor = Cursor::default();
    }
    if len == cursor.offset {
        return Vec::new();
    }
    if file.seek(SeekFrom::Start(cursor.offset)).is_err() {
        return Vec::new();
    }
    let mut chunk = Vec::new();
    if file.take(len - cursor.offset).read_to_end(&mut chunk).is_err() {
        return Vec::new();
    }
    cursor.offset += chunk.len() as u64;

    let mut buf = std::mem::take(&mut cursor.partial);
    buf.extend_from_slice(&chunk);
    let mut lines = Vec::new();
    let mut start = 0;
    while let Some(pos) = buf[start..].iter().position(|&b| b == b'\n') {
        lines.push(String::from_utf8_lossy(&buf[start..start + pos]).into_owned());
        start += pos + 1;
    }
    // The unfinished tail is not parsed. It is read again once the rest of the line arrives.
    cursor.partial = buf[start..].to_vec();
    lines
}

#[derive(Clone, Copy, Default)]
struct Message {
    at_ms: u64,
    input: u64,
    cache_creation: u64,
    output: u64,
    cache_read: u64,
    /// The model's input price against Sonnet 5.5. See `model_weight`.
    weight: f64,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Claude {
    /// Input, cache creation and output tokens of the last five hours. Cache reads are excluded.
    pub tokens_5h: u64,
    pub tokens_last_hour: u64,
    pub cache_read_5h: u64,
    pub messages_5h: u64,
    /// The weighted cost of the messages since the start of the block the caller asked for. Each model's
    /// messages are weighted by its price (see `model_weight`), in input-token units of Sonnet 5.5.
    pub window_units: f64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    /// The percent Codex reported. Zero when its window has reset since that report.
    pub used_percent: f64,
    pub window_minutes: u64,
    pub resets_at_ms: u64,
    /// When Codex wrote the report. The usage may have grown since then.
    pub observed_at_ms: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Codex {
    pub five_hour: Option<Window>,
    pub weekly: Option<Window>,
    pub plan: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub generated_at_ms: u64,
    pub claude: Claude,
    pub codex: Option<Codex>,
}

/// A model's input price against Sonnet 5.5's, which is $2 per million input tokens. Output costs five times input
/// in every model's price, so the same weight applies to a message's output. Unknown models, and the empty model
/// of a line that names none, count as Sonnet. Haiku 5.5 costs five times as much for a prompt over 100K tokens.
pub fn model_weight(model: &str, prompt_tokens: u64) -> f64 {
    if model.contains("fable") {
        5.0 // $10 input
    } else if model.contains("opus-5-5") {
        2.0 // $4 input
    } else if model.contains("opus") {
        2.5 // $5 input: Opus 5 and Opus 4.6 to 4.8
    } else if model.contains("sonnet-4") {
        1.5 // $3 input: Sonnet 4.6
    } else if model.contains("haiku-5") {
        if prompt_tokens > 100_000 { 0.25 } else { 0.05 } // $0.10 input, or $0.50 over 100K tokens
    } else if model.contains("haiku") {
        0.5 // $1 input: Haiku 4.5
    } else {
        1.0
    }
}

/// The start of the block the caller asks about: `since_ms` when given, kept within the last five hours. Without
/// one, the last five hours.
pub fn window_start(since_ms: Option<u64>, now: u64) -> u64 {
    let rolling = now.saturating_sub(WINDOW_MS);
    since_ms.map_or(rolling, |since| since.clamp(rolling, now))
}

impl Message {
    /// The message's cost in input-token units of Sonnet 5.5: input and cache writes at their prices, output at
    /// five times input, all times the model's weight. Cache reads are left out, as they are in the estimate.
    fn units(&self) -> f64 {
        self.weight * (self.input as f64 + CACHE_WRITE_RATIO * self.cache_creation as f64 + OUTPUT_RATIO * self.output as f64)
    }
}

impl Meter {
    /// Reads whatever was appended to recent transcripts, then totals the window ending at `now`.
    pub fn refresh(&mut self, claude_root: &Path, now: u64) -> Claude {
        let cutoff = now.saturating_sub(WINDOW_MS);
        let mut recent = Vec::new();
        collect_recent(claude_root, 0, cutoff, &mut recent);
        for path in &recent {
            self.read_new_lines(path);
        }
        // Files that fell out of the window no longer need a cursor. Their messages are pruned below.
        let live: HashSet<&PathBuf> = recent.iter().collect();
        self.files.retain(|path, _| live.contains(path));
        self.messages.retain(|_, m| m.at_ms >= cutoff);
        self.totals(now)
    }

    /// Reads what was appended to one transcript since the last read. Used for the usage bar's windows and for
    /// one session's total, so both count a message the same way.
    pub fn read_new_lines(&mut self, path: &Path) {
        let cursor = self.files.entry(path.to_path_buf()).or_default();
        let lines = read_appended(path, cursor);
        for line in &lines {
            self.apply_line(line);
        }
    }

    /// Applies one line of a transcript. Lines that are not assistant replies with usage are ignored.
    pub fn apply_line(&mut self, line: &str) {
        if !line.contains("\"usage\"") {
            return;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else { return };
        if value["type"] != "assistant" {
            return;
        }
        let Some(id) = value["message"]["id"].as_str() else { return };
        let usage = &value["message"]["usage"];
        if !usage.is_object() {
            return;
        }
        let Some(at_ms) = value["timestamp"].as_str().and_then(parse_iso_ms) else { return };
        let field = |name: &str| usage[name].as_u64().unwrap_or(0);
        let (input, cache_creation, cache_read) = (
            field("input_tokens"),
            field("cache_creation_input_tokens"),
            field("cache_read_input_tokens"),
        );
        let model = value["message"]["model"].as_str().unwrap_or("");
        let seen = Message {
            at_ms,
            input,
            cache_creation,
            output: field("output_tokens"),
            cache_read,
            // Set by the first line of the message. Later lines of the same message only raise its counts.
            weight: model_weight(model, input + cache_creation + cache_read),
        };
        // Streaming repeats a message with a growing output count. Keep the largest value of each field.
        let entry = self.messages.entry(id.to_string()).or_insert(seen);
        entry.at_ms = entry.at_ms.max(seen.at_ms);
        entry.input = entry.input.max(seen.input);
        entry.cache_creation = entry.cache_creation.max(seen.cache_creation);
        entry.output = entry.output.max(seen.output);
        entry.cache_read = entry.cache_read.max(seen.cache_read);
    }

    /// Every message read so far, counted once each: input, cache writes and output. Cache reads are left out,
    /// as they are in the usage bar.
    pub fn billed_tokens(&self) -> u64 {
        self.messages.values().map(|m| m.input + m.cache_creation + m.output).sum()
    }

    /// The weighted cost of the messages at or after `since`, counted once each.
    pub fn units_since(&self, since: u64) -> f64 {
        self.messages.values().filter(|m| m.at_ms >= since).map(Message::units).sum()
    }

    fn totals(&self, now: u64) -> Claude {
        let cutoff = now.saturating_sub(WINDOW_MS);
        let hour = now.saturating_sub(HOUR_MS);
        let mut claude = Claude::default();
        for m in self.messages.values().filter(|m| m.at_ms >= cutoff) {
            let billed = m.input + m.cache_creation + m.output;
            claude.tokens_5h += billed;
            claude.cache_read_5h += m.cache_read;
            claude.messages_5h += 1;
            if m.at_ms >= hour {
                claude.tokens_last_hour += billed;
            }
        }
        claude
    }
}

/// Every transcript modified since `cutoff`, found under `dir`.
fn collect_recent(dir: &Path, depth: usize, cutoff: u64, out: &mut Vec<PathBuf>) {
    if depth > CLAUDE_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            collect_recent(&path, depth + 1, cutoff, out);
        } else if path.extension().is_some_and(|ext| ext == "jsonl") && millis(meta.modified()) >= cutoff {
            out.push(path);
        }
    }
}

/// The newest rate-limit report Codex has written. Returns None when no rollout has one.
pub fn codex_limits(root: &Path, now: u64) -> Option<Codex> {
    let mut rollouts: Vec<(u64, PathBuf)> = Vec::new();
    collect_codex(root, 0, now.saturating_sub(CODEX_LOOKBACK_MS), &mut rollouts);
    rollouts.sort_by(|a, b| b.0.cmp(&a.0));
    let mut newest: Option<(u64, Value)> = None;
    for (_, path) in rollouts.iter().take(CODEX_FILES) {
        if let Some((at, report)) = last_codex_report(path) {
            if newest.as_ref().is_none_or(|(best, _)| at > *best) {
                newest = Some((at, report));
            }
        }
    }
    let (observed, report) = newest?;
    Some(Codex {
        five_hour: codex_window(&report["primary"], observed, now),
        weekly: codex_window(&report["secondary"], observed, now),
        plan: report["plan_type"].as_str().map(str::to_owned),
    })
}

fn collect_codex(dir: &Path, depth: usize, cutoff: u64, out: &mut Vec<(u64, PathBuf)>) {
    // Sessions are sorted into year/month/day folders, so the tree is only three levels deep.
    if depth > 3 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            collect_codex(&path, depth + 1, cutoff, out);
        } else if path.extension().is_some_and(|ext| ext == "jsonl") {
            let modified = millis(meta.modified());
            if modified >= cutoff {
                out.push((modified, path));
            }
        }
    }
}

/// The last `rate_limits` report in a rollout, read from its tail only.
fn last_codex_report(path: &Path) -> Option<(u64, Value)> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let from = len.saturating_sub(CODEX_TAIL_BYTES);
    file.seek(SeekFrom::Start(from)).ok()?;
    let mut tail = Vec::new();
    file.read_to_end(&mut tail).ok()?;
    let text = String::from_utf8_lossy(&tail);
    // The first line may start partway through, so it is skipped when the read began inside the file.
    let mut lines: Vec<&str> = text.lines().collect();
    if from > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    for line in lines.iter().rev() {
        if !line.contains("\"rate_limits\"") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
        let report = &value["payload"]["rate_limits"];
        if !report.is_object() {
            continue;
        }
        let at = value["timestamp"].as_str().and_then(parse_iso_ms)?;
        return Some((at, report.clone()));
    }
    None
}

/// One of Codex's windows, as its report describes it.
fn codex_window(raw: &Value, observed: u64, now: u64) -> Option<Window> {
    let window_minutes = raw["window_minutes"].as_u64()?;
    let resets_at_ms = raw["resets_at"].as_u64()? * 1000;
    let used_percent = raw["used_percent"].as_f64()?;
    Some(window_at(used_percent, window_minutes, resets_at_ms, observed, now))
}

/// A window as it stands at `now`. A report from before the window's reset is replaced by an unused
/// window, with the reset moved forward to the window that is open now. The reporter writes nothing
/// after a report, so nothing was used in the windows that followed it.
pub fn window_at(used_percent: f64, window_minutes: u64, resets_at_ms: u64, observed_at_ms: u64, now: u64) -> Window {
    let length = window_minutes * 60_000;
    if resets_at_ms > now || length == 0 {
        return Window { used_percent, window_minutes, resets_at_ms, observed_at_ms };
    }
    let resets_at_ms = resets_at_ms + ((now - resets_at_ms) / length + 1) * length;
    Window { used_percent: 0.0, window_minutes, resets_at_ms, observed_at_ms }
}

fn millis(time: std::io::Result<SystemTime>) -> u64 {
    time.ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64)
}

pub fn now_ms() -> u64 {
    millis(Ok(SystemTime::now()))
}

/// `2026-07-10T21:36:11.154Z` to Unix milliseconds. Only the UTC form that Claude and Codex write is accepted.
pub fn parse_iso_ms(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    if bytes.len() < 20 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' || bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }
    let num = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hour, minute, second) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let mut millis = 0;
    if bytes.get(19) == Some(&b'.') {
        let digits: String = text.get(20..).unwrap_or("").chars().take_while(|c| c.is_ascii_digit()).collect();
        let padded = format!("{:0<3}", &digits[..digits.len().min(3)]);
        millis = padded.parse::<i64>().ok()?;
    }
    let days = days_from_civil(year, month, day);
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second;
    u64::try_from(seconds * 1000 + millis).ok()
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn snapshot(meter: &Mutex<Meter>, claude_root: &Path, codex_root: &Path, now: u64, since: u64) -> Snapshot {
    let claude = {
        let mut meter = meter.lock().unwrap_or_else(|e| e.into_inner());
        let mut claude = meter.refresh(claude_root, now);
        claude.window_units = meter.units_since(since);
        claude
    };
    Snapshot { generated_at_ms: now, claude, codex: codex_limits(codex_root, now) }
}

#[tauri::command]
pub async fn usage_snapshot(state: tauri::State<'_, UsageState>, since_ms: Option<u64>) -> Result<Snapshot, String> {
    let meter = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Snapshot, String> {
        let home = dirs::home_dir().ok_or("Could not find the home folder")?;
        let now = now_ms();
        let since = window_start(since_ms, now);
        Ok(snapshot(&meter, &home.join(".claude/projects"), &home.join(".codex/sessions"), now, since))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A scratch folder that is removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("nesa-usage-{}-{}", std::process::id(), name));
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

    const NOW: u64 = 1_791_504_000_000; // 2026-10-09T00:00:00Z

    /// An assistant line as Claude Code writes it. Only the fields the meter reads are present.
    fn line(id: &str, at: &str, input: u64, cache_creation: u64, output: u64, cache_read: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{at}","message":{{"id":"{id}","usage":{{"input_tokens":{input},"cache_creation_input_tokens":{cache_creation},"output_tokens":{output},"cache_read_input_tokens":{cache_read}}}}}}}"#
        )
    }

    fn append(path: &Path, text: &str) {
        let mut file = fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        file.write_all(text.as_bytes()).unwrap();
    }

    fn iso(ms: u64) -> String {
        // Only used to build test lines, so it works from the epoch offsets the tests use.
        let secs = ms / 1000;
        let (days, rem) = (secs / 86_400, secs % 86_400);
        let (y, m, d) = civil_from_days(days as i64);
        format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z", rem / 3600, rem % 3600 / 60, rem % 60)
    }

    fn civil_from_days(z: i64) -> (i64, i64, i64) {
        let z = z + 719_468;
        let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (yoe + era * 400 + i64::from(m <= 2), m, d)
    }

    /// An assistant line that names its model, as Claude Code writes it.
    fn model_line(id: &str, at: &str, model: &str, input: u64, cache_creation: u64, output: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{at}","message":{{"id":"{id}","model":"{model}","usage":{{"input_tokens":{input},"cache_creation_input_tokens":{cache_creation},"output_tokens":{output},"cache_read_input_tokens":0}}}}}}"#
        )
    }

    #[test]
    fn each_model_is_weighted_by_its_input_price_against_sonnet() {
        assert_eq!(model_weight("claude-sonnet-5-5", 0), 1.0);
        assert_eq!(model_weight("claude-opus-5-5", 0), 2.0);
        assert_eq!(model_weight("claude-opus-4-7", 0), 2.5);
        assert_eq!(model_weight("claude-sonnet-4-6", 0), 1.5);
        assert_eq!(model_weight("claude-fable-5-1", 0), 5.0);
        assert_eq!(model_weight("claude-haiku-4-5", 0), 0.5);
        assert_eq!(model_weight("claude-haiku-5-5", 50_000), 0.05);
        assert_eq!(model_weight("claude-haiku-5-5", 150_000), 0.25, "Haiku 5.5 is priced higher over 100K prompt tokens");
        assert_eq!(model_weight("<synthetic>", 0), 1.0, "a line that names no model counts as Sonnet");
    }

    #[test]
    fn an_opus_message_costs_twice_a_sonnet_message_with_the_same_counts() {
        let at = iso(NOW - HOUR_MS);
        let mut opus = Meter::default();
        opus.apply_line(&model_line("a", &at, "claude-opus-5-5", 1000, 0, 200));
        let mut sonnet = Meter::default();
        sonnet.apply_line(&model_line("b", &at, "claude-sonnet-5-5", 1000, 0, 200));
        assert_eq!(sonnet.units_since(0), 1000.0 + 5.0 * 200.0);
        assert_eq!(opus.units_since(0), 2.0 * sonnet.units_since(0));
    }

    #[test]
    fn cache_writes_cost_a_quarter_more_than_input_and_output_five_times() {
        let at = iso(NOW - HOUR_MS);
        let mut meter = Meter::default();
        meter.apply_line(&model_line("c", &at, "claude-sonnet-5-5", 0, 100, 10));
        assert_eq!(meter.units_since(0), 100.0 * 1.25 + 10.0 * 5.0);
    }

    #[test]
    fn the_block_counts_only_messages_at_or_after_its_start() {
        let mut meter = Meter::default();
        meter.apply_line(&model_line("old", &iso(NOW - 4 * HOUR_MS), "claude-sonnet-5-5", 1000, 0, 0));
        meter.apply_line(&model_line("new", &iso(NOW - HOUR_MS), "claude-sonnet-5-5", 300, 0, 0));
        assert_eq!(meter.units_since(NOW - 3 * HOUR_MS), 300.0);
        assert_eq!(meter.units_since(0), 1300.0);
    }

    #[test]
    fn the_block_start_stays_within_the_last_five_hours() {
        assert_eq!(window_start(None, NOW), NOW - WINDOW_MS);
        assert_eq!(window_start(Some(NOW - 6 * HOUR_MS), NOW), NOW - WINDOW_MS, "older than the window is clamped");
        assert_eq!(window_start(Some(NOW - HOUR_MS), NOW), NOW - HOUR_MS);
        assert_eq!(window_start(Some(NOW + 1), NOW), NOW, "a start after now is clamped");
    }

    #[test]
    fn iso_timestamps_match_python() {
        // Python: datetime(2026,7,10,21,36,11,154000,tzinfo=utc).timestamp()*1000
        assert_eq!(parse_iso_ms("2026-07-10T21:36:11.154Z"), Some(1_783_719_371_154));
        // Python: datetime(2026,10,9,tzinfo=utc).timestamp()*1000
        assert_eq!(parse_iso_ms("2026-10-09T00:00:00Z"), Some(NOW));
        assert_eq!(parse_iso_ms("2026-10-09T00:00:00.5Z"), Some(NOW + 500));
        assert_eq!(parse_iso_ms("not a time"), None);
    }

    #[test]
    fn a_message_streamed_in_several_lines_counts_once_at_its_largest_values() {
        let scratch = Scratch::new("dedupe");
        let file = scratch.0.join("session.jsonl");
        let at = iso(NOW - HOUR_MS);
        append(&file, &format!("{}\n", line("msg_1", &at, 100, 50, 10, 900)));
        append(&file, &format!("{}\n", line("msg_1", &at, 100, 50, 300, 900)));
        append(&file, &format!("{}\n", line("msg_1", &at, 100, 50, 1773, 900)));
        let mut meter = Meter::default();
        let claude = meter.refresh(&scratch.0, NOW);
        assert_eq!(claude.messages_5h, 1);
        assert_eq!(claude.tokens_5h, 100 + 50 + 1773);
        assert_eq!(claude.cache_read_5h, 900);
    }

    #[test]
    fn a_message_copied_into_another_file_is_still_counted_once() {
        let scratch = Scratch::new("forked");
        let at = iso(NOW - HOUR_MS);
        append(&scratch.0.join("original.jsonl"), &format!("{}\n", line("msg_9", &at, 10, 0, 20, 0)));
        append(&scratch.0.join("forked.jsonl"), &format!("{}\n", line("msg_9", &at, 10, 0, 20, 0)));
        let claude = Meter::default().refresh(&scratch.0, NOW);
        assert_eq!(claude.messages_5h, 1);
        assert_eq!(claude.tokens_5h, 30);
    }

    #[test]
    fn messages_older_than_five_hours_are_left_out_and_pruned() {
        let scratch = Scratch::new("window");
        let file = scratch.0.join("session.jsonl");
        append(&file, &format!("{}\n", line("old", &iso(NOW - 6 * HOUR_MS), 999, 0, 999, 0)));
        append(&file, &format!("{}\n", line("new", &iso(NOW - HOUR_MS), 7, 0, 3, 0)));
        let mut meter = Meter::default();
        let claude = meter.refresh(&scratch.0, NOW);
        assert_eq!(claude.tokens_5h, 10);
        assert_eq!(claude.tokens_last_hour, 10);
        assert_eq!(meter.messages.len(), 1, "the old message is pruned");
    }

    #[test]
    fn a_line_without_its_newline_waits_for_the_rest() {
        let scratch = Scratch::new("partial");
        let file = scratch.0.join("session.jsonl");
        let full = format!("{}\n", line("msg_2", &iso(NOW - HOUR_MS), 40, 0, 60, 0));
        let (head, tail) = full.split_at(full.len() / 2);
        append(&file, head);
        let mut meter = Meter::default();
        assert_eq!(meter.refresh(&scratch.0, NOW).messages_5h, 0, "half a line is not parsed");
        append(&file, tail);
        let claude = meter.refresh(&scratch.0, NOW);
        assert_eq!(claude.messages_5h, 1);
        assert_eq!(claude.tokens_5h, 100);
    }

    #[test]
    fn a_file_that_shrinks_is_read_again_from_the_top() {
        let scratch = Scratch::new("shrink");
        let file = scratch.0.join("session.jsonl");
        append(&file, &format!("{}\n{}\n", line("a", &iso(NOW - HOUR_MS), 1, 0, 1, 0), line("b", &iso(NOW - HOUR_MS), 1, 0, 1, 0)));
        let mut meter = Meter::default();
        assert_eq!(meter.refresh(&scratch.0, NOW).messages_5h, 2);
        fs::write(&file, format!("{}\n", line("c", &iso(NOW - HOUR_MS), 5, 0, 5, 0))).unwrap();
        let claude = meter.refresh(&scratch.0, NOW);
        assert_eq!(claude.messages_5h, 3, "the replacement line is read, and the earlier ones are kept");
        assert_eq!(claude.tokens_5h, 1 + 1 + 1 + 1 + 5 + 5);
    }

    fn codex_report(observed: &str, resets_at_s: u64, used: f64) -> String {
        format!(
            r#"{{"timestamp":"{observed}","type":"event_msg","payload":{{"type":"token_count","info":null,"rate_limits":{{"primary":{{"used_percent":{used},"window_minutes":300,"resets_at":{resets_at_s}}},"secondary":{{"used_percent":7.0,"window_minutes":10080,"resets_at":{}}},"plan_type":"plus"}}}}}}"#,
            resets_at_s + 4 * 24 * 3600
        )
    }

    #[test]
    fn an_open_codex_window_keeps_the_percent_it_reported() {
        let scratch = Scratch::new("codex-open");
        let day = scratch.0.join("2026/10/09");
        fs::create_dir_all(&day).unwrap();
        let resets = NOW / 1000 + 2 * 3600;
        append(&day.join("rollout-1.jsonl"), &format!("{}\n", codex_report(&iso(NOW - 600_000), resets, 42.0)));
        let codex = codex_limits(&scratch.0, NOW).expect("a report exists");
        let five = codex.five_hour.expect("five-hour window");
        assert_eq!(five.used_percent, 42.0);
        assert_eq!(five.resets_at_ms, resets * 1000);
        assert_eq!(codex.plan.as_deref(), Some("plus"));
        assert_eq!(codex.weekly.expect("weekly window").used_percent, 7.0);
    }

    #[test]
    fn a_codex_window_that_reset_since_the_report_reads_as_unused() {
        let scratch = Scratch::new("codex-stale");
        let day = scratch.0.join("2026/10/01");
        fs::create_dir_all(&day).unwrap();
        // The report is from before its window closed. Nothing has been written since.
        let resets = NOW / 1000 - 3 * 3600;
        append(&day.join("rollout-1.jsonl"), &format!("{}\n", codex_report(&iso(NOW - 4 * HOUR_MS), resets, 88.0)));
        let five = codex_limits(&scratch.0, NOW).unwrap().five_hour.unwrap();
        assert_eq!(five.used_percent, 0.0, "the old 88% belonged to a window that has closed");
        assert!(five.resets_at_ms > NOW, "the reset moves to the window that is open now");
        // One five-hour step past the old reset: that is the window that is open now.
        assert_eq!(five.resets_at_ms, (resets + 5 * 3600) * 1000);
    }

    #[test]
    fn the_newest_codex_report_wins_across_rollouts() {
        let scratch = Scratch::new("codex-newest");
        let day = scratch.0.join("2026/10/09");
        fs::create_dir_all(&day).unwrap();
        let resets = NOW / 1000 + 3600;
        append(&day.join("older.jsonl"), &format!("{}\n", codex_report(&iso(NOW - 3_600_000), resets, 10.0)));
        append(&day.join("newer.jsonl"), &format!("{}\n", codex_report(&iso(NOW - 60_000), resets, 25.0)));
        let five = codex_limits(&scratch.0, NOW).unwrap().five_hour.unwrap();
        assert_eq!(five.used_percent, 25.0);
        assert_eq!(five.observed_at_ms, NOW - 60_000);
    }

    /// Prints the totals of the real transcripts for a fixed `NESA_USAGE_NOW_MS`, to compare with a script.
    /// Run with: `NESA_USAGE_NOW_MS=... cargo test real_transcripts -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_transcripts() {
        let now: u64 = std::env::var("NESA_USAGE_NOW_MS").unwrap().parse().unwrap();
        let home = dirs::home_dir().unwrap();
        let claude = Meter::default().refresh(&home.join(".claude/projects"), now);
        println!("{claude:?}");
    }
}
