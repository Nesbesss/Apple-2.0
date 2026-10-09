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
}
