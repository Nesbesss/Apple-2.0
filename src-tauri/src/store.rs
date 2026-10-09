use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Claude,
    Codex,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Claude => "Claude",
            Kind::Codex => "Codex",
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str, name: &str) -> Session {
        Session { id: id.into(), kind: Kind::Claude, name: name.into(), cwd: PathBuf::from("/tmp"), agent_id: None }
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
    fn default_name_uses_the_folder_and_numbers_duplicates() {
        assert_eq!(default_name("nesa", &[]), "nesa");
        assert_eq!(default_name("nesa", &["other"]), "nesa");
        assert_eq!(default_name("nesa", &["nesa"]), "nesa 2");
        assert_eq!(default_name("nesa", &["nesa", "nesa 2"]), "nesa 3");
    }
}
