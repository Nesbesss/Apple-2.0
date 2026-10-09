use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Claude,
    Codex,
    /// A plain login shell in a folder, with no agent in it.
    Terminal,
    /// A web page in a native webview. It has no process.
    Browser,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Claude => "Claude",
            Kind::Codex => "Codex",
            Kind::Terminal => "Terminal",
            Kind::Browser => "Browser",
        }
    }

    /// Whether the session runs a process behind a pseudo-terminal. A browser does not.
    pub fn has_pty(self) -> bool {
        self != Kind::Browser
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Session {
    /// App-level id, stable across restarts.
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub cwd: PathBuf,
    /// The agent's own session id. Claude: same as `id`. Codex: discovered after launch.
    #[serde(default)]
    pub agent_id: Option<String>,
    /// The page a browser session shows. Other kinds leave it unset.
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Group {
    pub name: String,
    pub sessions: Vec<Session>,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Store {
    pub groups: Vec<Group>,
}

impl Store {
    fn path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("SessionDeck")
            .join("groups.json")
    }

    /// Loads saved groups. A file that fails to parse is moved aside instead of being overwritten.
    pub fn load() -> Self {
        let path = Self::path();
        let Ok(bytes) = fs::read(&path) else {
            return Self::default();
        };
        match serde_json::from_slice(&bytes) {
            Ok(store) => store,
            Err(err) => {
                eprintln!("groups.json is unreadable ({err}); moving it aside");
                let _ = fs::rename(&path, path.with_extension("json.corrupt"));
                Self::default()
            }
        }
    }

    pub fn save(&self) -> io::Result<()> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self).map_err(io::Error::other)?)?;
        fs::rename(tmp, path)
    }

    /// Index of the group that holds session `id`, if any.
    pub fn group_of(&self, id: &str) -> Option<usize> {
        self.groups.iter().position(|g| g.sessions.iter().any(|s| s.id == id))
    }

    /// Renames a session. The name is trimmed and must fit in `MAX_NAME_CHARS`.
    pub fn rename_session(&mut self, id: &str, name: &str) -> Result<(), String> {
        let name = clean_name(name)?;
        let session = self
            .groups
            .iter_mut()
            .flat_map(|g| g.sessions.iter_mut())
            .find(|s| s.id == id)
            .ok_or("That session no longer exists")?;
        session.name = name;
        Ok(())
    }

    /// Moves a session to the end of group `to`. Its agent keeps running: processes are keyed by session id.
    pub fn move_session(&mut self, id: &str, to: usize) -> Result<(), String> {
        if to >= self.groups.len() {
            return Err("That group no longer exists".into());
        }
        let from = self.group_of(id).ok_or("That session no longer exists")?;
        if from == to {
            return Ok(());
        }
        let position = self.groups[from].sessions.iter().position(|s| s.id == id).ok_or("That session no longer exists")?;
        let session = self.groups[from].sessions.remove(position);
        self.groups[to].sessions.push(session);
        Ok(())
    }

    /// Puts session `id` into group `to`, before the session now at `index`, or last when `index` is past the
    /// end. Within one group the index refers to the list as it was before the move. Its agent keeps running.
    pub fn place_session(&mut self, id: &str, to: usize, index: usize) -> Result<(), String> {
        if to >= self.groups.len() {
            return Err("That group no longer exists".into());
        }
        let from = self.group_of(id).ok_or("That session no longer exists")?;
        let position = self.groups[from].sessions.iter().position(|s| s.id == id).ok_or("That session no longer exists")?;
        let mut index = index.min(self.groups[to].sessions.len());
        if from == to {
            if index == position || index == position + 1 {
                return Ok(()); // dropped where it already is
            }
            if position < index {
                index -= 1; // the session leaves its place before the index is used
            }
        }
        let session = self.groups[from].sessions.remove(position);
        self.groups[to].sessions.insert(index, session);
        Ok(())
    }

    /// Moves group `from` so that it sits before the group now at `index`, or last when `index` is past the
    /// end. The index refers to the list as it was before the move.
    pub fn move_group(&mut self, from: usize, index: usize) -> Result<(), String> {
        if from >= self.groups.len() {
            return Err("That group no longer exists".into());
        }
        if index == from || index == from + 1 {
            return Ok(());
        }
        let group = self.groups.remove(from);
        let index = if from < index { index - 1 } else { index };
        self.groups.insert(index.min(self.groups.len()), group);
        Ok(())
    }
}

pub const MAX_NAME_CHARS: usize = 80;

fn clean_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Name cannot be empty".into());
    }
    if trimmed.chars().count() > MAX_NAME_CHARS {
        return Err(format!("Names can be at most {MAX_NAME_CHARS} characters"));
    }
    Ok(trimmed.to_string())
}

/// Name for a new session: its folder name, numbered when the group already uses it.
pub fn default_name(folder: &str, taken: &[&str]) -> String {
    if !taken.contains(&folder) {
        return folder.to_string();
    }
    (2..)
        .map(|n| format!("{folder} {n}"))
        .find(|candidate| !taken.contains(&candidate.as_str()))
        .expect("an unused number always exists")
}

/// Names for `count` sessions opened together from one base: each is numbered around the names in use
/// and the names given earlier in the same batch.
pub fn batch_names(base: &str, taken: &[&str], count: usize) -> Vec<String> {
    let mut names: Vec<String> = Vec::with_capacity(count);
    for _ in 0..count {
        let in_use: Vec<&str> = taken.iter().copied().chain(names.iter().map(String::as_str)).collect();
        names.push(default_name(base, &in_use));
    }
    names
}

/// Names for sessions a person did not name: short and easy to say, so the sessions tell apart at a glance.
const FRIENDLY_NAMES: &[&str] = &[
    "jerry", "otis", "mabel", "pip", "ruby", "milo", "hazel", "theo", "nora", "biscuit", "olive", "gus", "wren",
    "sully", "juniper", "felix", "rosie", "ziggy", "maple", "kit", "bruno", "luna", "clem", "pearl", "moss", "rory",
    "tilly", "wally", "dot", "fern", "ned", "ollie", "sage", "tuck", "mango", "pickle", "birdie", "rex", "hank", "lulu",
];

/// A friendly name that is not in use, starting at `pick` and walking the list. When every name is taken,
/// the picked name is numbered the way folder names are.
pub fn random_name(taken: &[&str], pick: usize) -> String {
    let len = FRIENDLY_NAMES.len();
    (0..len)
        .map(|offset| FRIENDLY_NAMES[(pick + offset) % len])
        .find(|name| !taken.contains(name))
        .map(str::to_string)
        .unwrap_or_else(|| default_name(FRIENDLY_NAMES[pick % len], taken))
}

/// Names for `count` sessions opened together without a typed name. Each one avoids the names before it.
pub fn random_names(taken: &[&str], count: usize, seed: usize) -> Vec<String> {
    let mut names: Vec<String> = Vec::with_capacity(count);
    for i in 0..count {
        let in_use: Vec<&str> = taken.iter().copied().chain(names.iter().map(String::as_str)).collect();
        names.push(random_name(&in_use, seed.wrapping_add(i * 7)));
    }
    names
}

#[cfg(test)]
mod order_tests {
    use super::*;

    fn session(id: &str) -> Session {
        Session { id: id.into(), kind: Kind::Claude, name: id.into(), cwd: PathBuf::from("/tmp"), agent_id: None, url: None }
    }

    /// Group A holds a1 and a2, group B holds b1, group C is empty.
    fn store() -> Store {
        Store {
            groups: vec![
                Group { name: "A".into(), sessions: vec![session("a1"), session("a2")] },
                Group { name: "B".into(), sessions: vec![session("b1")] },
                Group { name: "C".into(), sessions: vec![] },
            ],
        }
    }

    fn sessions(store: &Store, group: usize) -> Vec<&str> {
        store.groups[group].sessions.iter().map(|s| s.id.as_str()).collect()
    }

    fn groups(store: &Store) -> Vec<&str> {
        store.groups.iter().map(|g| g.name.as_str()).collect()
    }

    #[test]
    fn a_session_moves_before_the_chosen_position_within_its_group() {
        let mut store = store();
        store.place_session("a2", 0, 0).unwrap();
        assert_eq!(sessions(&store, 0), vec!["a2", "a1"]);
        store.place_session("a2", 0, 2).unwrap();
        assert_eq!(sessions(&store, 0), vec!["a1", "a2"]);
    }

    #[test]
    fn dropping_a_session_where_it_already_is_changes_nothing() {
        let mut store = store();
        store.place_session("a1", 0, 0).unwrap();
        store.place_session("a1", 0, 1).unwrap();
        assert_eq!(sessions(&store, 0), vec!["a1", "a2"]);
    }

    #[test]
    fn a_session_can_move_into_another_group_at_a_position() {
        let mut store = store();
        store.place_session("a1", 1, 0).unwrap();
        assert_eq!(sessions(&store, 0), vec!["a2"]);
        assert_eq!(sessions(&store, 1), vec!["a1", "b1"]);
        store.place_session("b1", 2, 99).unwrap();
        assert_eq!(sessions(&store, 2), vec!["b1"], "past the end means last");
    }

    #[test]
    fn placing_into_a_missing_group_is_refused() {
        let mut store = store();
        assert!(store.place_session("a1", 9, 0).is_err());
        assert!(store.place_session("missing", 0, 0).is_err());
        assert_eq!(sessions(&store, 0), vec!["a1", "a2"]);
    }

    #[test]
    fn a_group_moves_before_the_chosen_position() {
        let mut store = store();
        store.move_group(0, 3).unwrap();
        assert_eq!(groups(&store), vec!["B", "C", "A"], "to the end");
        store.move_group(2, 0).unwrap();
        assert_eq!(groups(&store), vec!["A", "B", "C"], "to the front");
        store.move_group(1, 2).unwrap();
        assert_eq!(groups(&store), vec!["A", "B", "C"], "onto its own place changes nothing");
        assert!(store.move_group(7, 0).is_err());
    }

    #[test]
    fn random_names_skip_names_in_use_and_stay_unique_in_a_batch() {
        assert_eq!(random_name(&[], 0), FRIENDLY_NAMES[0]);
        assert_eq!(random_name(&[FRIENDLY_NAMES[0]], 0), FRIENDLY_NAMES[1]);
        let names = random_names(&[], 6, 5);
        let unique: std::collections::HashSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), 6);
    }

    #[test]
    fn when_every_friendly_name_is_taken_the_name_is_numbered() {
        let all: Vec<&str> = FRIENDLY_NAMES.to_vec();
        assert_eq!(random_name(&all, 0), format!("{} 2", FRIENDLY_NAMES[0]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str, name: &str) -> Session {
        Session { id: id.into(), kind: Kind::Claude, name: name.into(), cwd: PathBuf::from("/tmp"), agent_id: None, url: None }
    }

    fn store() -> Store {
        Store {
            groups: vec![
                Group { name: "A".into(), sessions: vec![session("a1", "one"), session("a2", "two")] },
                Group { name: "B".into(), sessions: vec![session("b1", "three")] },
                Group { name: "C".into(), sessions: vec![] },
            ],
        }
    }

    fn ids(store: &Store, group: usize) -> Vec<&str> {
        store.groups[group].sessions.iter().map(|s| s.id.as_str()).collect()
    }

    #[test]
    fn rename_trims_and_touches_only_the_target() {
        let mut s = store();
        s.rename_session("a2", "  Review  ").unwrap();
        assert_eq!(s.groups[0].sessions[1].name, "Review");
        assert_eq!(s.groups[0].sessions[0].name, "one");
    }

    #[test]
    fn rename_rejects_blank_long_and_unknown() {
        let mut s = store();
        assert!(s.rename_session("a1", "   ").is_err());
        assert!(s.rename_session("a1", &"x".repeat(MAX_NAME_CHARS + 1)).is_err());
        assert!(s.rename_session("a1", &"x".repeat(MAX_NAME_CHARS)).is_ok());
        assert!(s.rename_session("missing", "Name").is_err());
    }

    #[test]
    fn move_appends_to_target_and_keeps_the_session() {
        let mut s = store();
        s.move_session("a1", 1).unwrap();
        assert_eq!(ids(&s, 0), vec!["a2"]);
        assert_eq!(ids(&s, 1), vec!["b1", "a1"]);
        assert_eq!(s.group_of("a1"), Some(1));
        assert_eq!(s.groups[1].sessions[1].name, "one");
    }

    #[test]
    fn move_to_same_group_is_a_no_op() {
        let mut s = store();
        s.move_session("a1", 0).unwrap();
        assert_eq!(ids(&s, 0), vec!["a1", "a2"]);
    }

    #[test]
    fn move_rejects_unknown_session_or_group() {
        let mut s = store();
        assert!(s.move_session("a1", 9).is_err());
        assert!(s.move_session("missing", 1).is_err());
        assert_eq!(ids(&s, 0), vec!["a1", "a2"]);
    }

    #[test]
    fn move_into_empty_group() {
        let mut s = store();
        s.move_session("b1", 2).unwrap();
        assert!(ids(&s, 1).is_empty());
        assert_eq!(ids(&s, 2), vec!["b1"]);
    }

    #[test]
    fn batch_names_number_around_names_in_use() {
        assert_eq!(batch_names("nesa", &[], 3), vec!["nesa", "nesa 2", "nesa 3"]);
        assert_eq!(batch_names("nesa", &["nesa", "nesa 3"], 2), vec!["nesa 2", "nesa 4"]);
        assert_eq!(batch_names("chess", &["other"], 1), vec!["chess"]);
    }

    #[test]
    fn default_name_uses_the_folder_and_numbers_duplicates() {
        assert_eq!(default_name("nesa", &[]), "nesa");
        assert_eq!(default_name("nesa", &["other"]), "nesa");
        assert_eq!(default_name("nesa", &["nesa"]), "nesa 2");
        assert_eq!(default_name("nesa", &["nesa", "nesa 2"]), "nesa 3");
    }
}
