//! Image files a session prints as paths. The terminal turns those paths into links, and the viewer shows them.

use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    collections::VecDeque,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

/// Largest image the viewer loads. Bigger files are refused rather than sent over IPC.
pub const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;
/// Folders a bare file name is searched through, and how far down, so a huge folder cannot stall a lookup.
const SEARCH_DEPTH: usize = 4;
const SEARCH_ENTRIES: usize = 20_000;
/// Longest text that is treated as a path. Anything longer is a line of output, not a file name.
const MAX_PATH_CHARS: usize = 1024;

const IMAGE_TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("bmp", "image/bmp"),
    ("svg", "image/svg+xml"),
];

fn mime_for(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    IMAGE_TYPES.iter().find(|(known, _)| *known == ext).map(|(_, mime)| *mime)
}

/// The image file a path from the terminal refers to, or None. Agents print paths as absolute, as `~/`,
/// relative to the session's folder, as a bare file name, or shortened with `…`, as in `~/Desktop/…/shot.png`.
pub fn resolve(cwd: &Path, text: &str) -> Option<PathBuf> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > MAX_PATH_CHARS {
        return None;
    }
    let home = dirs::home_dir().unwrap_or_default();

    // A shortened path keeps its start and its end. The start names a folder to search, then the folder
    // the session runs in. The end is the file name.
    if let Some((head, tail)) = text.rsplit_once('…') {
        let name = tail.rsplit('/').next().unwrap_or(tail);
        let head = head.trim_end_matches('/');
        let folder = if head.is_empty() {
            cwd.to_path_buf()
        } else {
            let start = expand(head, &home);
            if start.is_absolute() { start } else { cwd.join(start) }
        };
        return find_named(&folder, name).or_else(|| find_named(cwd, name));
    }

    let path = expand(text, &home);
    if path.is_absolute() {
        return image_file(&path);
    }
    if let Some(found) = image_file(&cwd.join(&path)) {
        return Some(found);
    }
    if !text.contains('/') {
        return find_named(cwd, text);
    }
    None
}

fn expand(text: &str, home: &Path) -> PathBuf {
    match text.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(text),
    }
}

/// The path, canonical, if it is an image file that exists.
fn image_file(path: &Path) -> Option<PathBuf> {
    if mime_for(path).is_none() || !path.is_file() {
        return None;
    }
    Some(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()))
}

/// The nearest image called `name` under `dir`. Breadth first, so a file near the top wins. Hidden folders,
/// `node_modules` and `target` are skipped, and the search stops after `SEARCH_ENTRIES` entries.
fn find_named(dir: &Path, name: &str) -> Option<PathBuf> {
    if name.is_empty() || !dir.is_dir() {
        return None;
    }
    let mut seen = 0;
    let mut queue = VecDeque::from([(dir.to_path_buf(), 0usize)]);
    while let Some((folder, depth)) = queue.pop_front() {
        let Ok(entries) = fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            seen += 1;
            if seen > SEARCH_ENTRIES {
                return None;
            }
            let path = entry.path();
            let file_name = entry.file_name();
            if path.is_dir() {
                if depth < SEARCH_DEPTH && !is_skipped(&file_name) {
                    queue.push_back((path, depth + 1));
                }
            } else if file_name.to_str() == Some(name) {
                if let Some(found) = image_file(&path) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn is_skipped(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    name.starts_with('.') || name == "node_modules" || name == "target"
}

/// The image at `path` as a data URL, for the viewer.
pub fn data_url(path: &Path) -> Result<String, String> {
    data_url_within(path, MAX_IMAGE_BYTES)
}

fn data_url_within(path: &Path, limit: u64) -> Result<String, String> {
    let mime = mime_for(path).ok_or("Only image files can be shown")?;
    let meta = fs::metadata(path).map_err(|_| "That image is no longer there".to_string())?;
    if !meta.is_file() {
        return Err("That image is no longer there".into());
    }
    if meta.len() > limit {
        return Err(format!("That image is larger than {} MB", limit / (1024 * 1024)));
    }
    let bytes = fs::read(path).map_err(|e| format!("Could not read the image: {e}"))?;
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    /// A fresh folder under the system temp dir, removed when the test ends.
    struct Folder(PathBuf);

    impl Folder {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("nesa-images-{}", Uuid::new_v4()));
            fs::create_dir_all(&dir).unwrap();
            Folder(dir)
        }
        fn file(&self, relative: &str) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"not really a png").unwrap();
            path
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn absolute_and_relative_paths_resolve_to_the_file() {
        let folder = Folder::new();
        let shot = folder.file("shots/a.png");
        let canonical = shot.canonicalize().unwrap();
        assert_eq!(resolve(&folder.0, shot.to_str().unwrap()), Some(canonical.clone()));
        assert_eq!(resolve(&folder.0, "shots/a.png"), Some(canonical));
    }

    #[test]
    fn a_bare_file_name_is_found_in_the_session_folder() {
        let folder = Folder::new();
        folder.file("deep/er/still/shot.png");
        let found = resolve(&folder.0, "shot.png").expect("found");
        assert!(found.ends_with("shot.png"));
    }

    #[test]
    fn a_shortened_path_resolves_by_its_file_name() {
        let folder = Folder::new();
        folder.file("out/chart.png");
        assert!(resolve(&folder.0, "…/chart.png").is_some());
        assert!(resolve(&folder.0, "out/…/chart.png").is_some());
        assert!(resolve(&folder.0, "/gone/…/chart.png").is_some(), "falls back to the session folder");
    }

    #[test]
    fn hidden_and_build_folders_are_not_searched() {
        let folder = Folder::new();
        folder.file(".cache/hidden.png");
        folder.file("target/built.png");
        assert!(resolve(&folder.0, "hidden.png").is_none());
        assert!(resolve(&folder.0, "built.png").is_none());
    }

    #[test]
    fn only_image_files_resolve() {
        let folder = Folder::new();
        folder.file("notes.txt");
        assert!(resolve(&folder.0, "notes.txt").is_none());
        assert!(resolve(&folder.0, "missing.png").is_none());
        assert!(resolve(&folder.0, "").is_none());
    }

    #[test]
    fn a_data_url_carries_the_image_type() {
        let folder = Folder::new();
        let path = folder.file("tiny.PNG");
        assert!(data_url(&path).unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn a_large_image_is_refused_and_a_text_file_is_not_an_image() {
        let folder = Folder::new();
        let path = folder.file("big.jpg");
        assert!(data_url_within(&path, 4).unwrap_err().contains("larger"));
        assert!(data_url(&folder.file("readme.md")).is_err());
    }
}
