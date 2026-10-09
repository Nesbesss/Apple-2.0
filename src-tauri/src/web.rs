//! Addresses for the built-in browser. Whatever the user types becomes an http or https URL, or a search.

use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::{
    webview::{NewWindowResponse, PageLoadEvent},
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Url, Webview, WebviewBuilder, WebviewUrl,
};

/// Where a browser page sits, in window coordinates (CSS pixels), and whether it shows at all.
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub visible: bool,
}

/// A browser session's frame, keyed by its session id.
#[derive(Deserialize)]
pub struct BrowserFrame {
    pub id: String,
    #[serde(flatten)]
    pub frame: Frame,
}

/// What the UI hears when a page moves: which session, and the address it now shows.
#[derive(Serialize, Clone)]
struct PageEvent {
    id: String,
    url: String,
}

/// The webview label of a browser session. Labels are how a page is found again from its session.
fn label(id: &str) -> String {
    format!("browser-{id}")
}

/// Places a page at `frame`, or hides it. A page is only shown when it has room to be seen.
fn apply_frame(webview: &Webview, frame: Frame) -> Result<(), String> {
    if !frame.visible || frame.width < 1.0 || frame.height < 1.0 {
        return webview.hide().map_err(|e| e.to_string());
    }
    webview
        .set_bounds(tauri::Rect {
            position: LogicalPosition::new(frame.x, frame.y).into(),
            size: LogicalSize::new(frame.width, frame.height).into(),
        })
        .map_err(|e| e.to_string())?;
    webview.show().map_err(|e| e.to_string())
}

/// Moves and shows or hides the browser pages the UI reports. A session with no page yet is skipped.
#[tauri::command]
pub fn browser_sync(items: Vec<BrowserFrame>, app: AppHandle) {
    for item in items {
        if let Some(webview) = app.get_webview(&label(&item.id)) {
            let _ = apply_frame(&webview, item.frame);
        }
    }
}

/// Opens `url` in browser session `id`, creating its page the first time, and places it at `frame`.
/// Returns the address that was opened, after normalizing what was typed.
#[tauri::command]
pub fn browser_navigate(id: String, url: String, frame: Frame, app: AppHandle) -> Result<String, String> {
    let address = normalize_url(&url)?;
    match app.get_webview(&label(&id)) {
        Some(webview) => {
            webview.navigate(address.clone()).map_err(|e| e.to_string())?;
            apply_frame(&webview, frame)?;
        }
        None => create_page(&app, &id, address.clone(), frame)?,
    }
    remember_url(&app, &id, address.as_str());
    Ok(address.to_string())
}

fn create_page(app: &AppHandle, id: &str, address: Url, frame: Frame) -> Result<(), String> {
    let window = app.get_window("main").ok_or("The main window is not open")?;
    let events = app.clone();
    let session = id.to_string();
    let page = WebviewBuilder::new(label(id), WebviewUrl::External(address))
        // Only web pages. A page cannot move itself to a file, or to the app's own webview.
        .on_navigation(|url| matches!(url.scheme(), "http" | "https"))
        // A page that opens a window does not get one. Its address bar stays the only way to move.
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_page_load(move |_webview, payload| {
            let url = payload.url().to_string();
            let _ = events.emit("browser-page", PageEvent { id: session.clone(), url: url.clone() });
            if matches!(payload.event(), PageLoadEvent::Finished) {
                remember_url(&events, &session, &url);
            }
        });
    let webview = window
        .add_child(page, LogicalPosition::new(frame.x, frame.y), LogicalSize::new(frame.width.max(1.0), frame.height.max(1.0)))
        .map_err(|e| e.to_string())?;
    apply_frame(&webview, frame)
}

/// Keeps the address a browser session shows, so the session opens there again.
fn remember_url(app: &AppHandle, id: &str, url: &str) {
    let state = app.state::<AppState>();
    let mut store = state.store.lock().unwrap();
    let mut changed = false;
    for session in store.groups.iter_mut().flat_map(|g| g.sessions.iter_mut()) {
        if session.id == id && session.url.as_deref() != Some(url) {
            session.url = Some(url.to_string());
            changed = true;
        }
    }
    if changed {
        let _ = store.save();
    }
}

/// Runs a script in a browser page, for back, forward and reload.
fn run_in_page(app: &AppHandle, id: &str, script: &str) -> Result<(), String> {
    let webview = app.get_webview(&label(id)).ok_or("That page is not open")?;
    webview.eval(script).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn browser_back(id: String, app: AppHandle) -> Result<(), String> {
    run_in_page(&app, &id, "history.back()")
}

#[tauri::command]
pub fn browser_forward(id: String, app: AppHandle) -> Result<(), String> {
    run_in_page(&app, &id, "history.forward()")
}

#[tauri::command]
pub fn browser_reload(id: String, app: AppHandle) -> Result<(), String> {
    run_in_page(&app, &id, "location.reload()")
}

/// Closes a session's page when the session goes: removed, or its group deleted.
pub fn close_browser(app: &AppHandle, id: &str) {
    if let Some(webview) = app.get_webview(&label(id)) {
        let _ = webview.close();
    }
}

/// Turns what the user typed into a page to open. A bare host gets https, and text that is not an
/// address becomes a search. Only web addresses are accepted, so a `javascript:` or `file:` address
/// never reaches a page.
pub fn normalize_url(input: &str) -> Result<Url, String> {
    let text = input.trim();
    if text.is_empty() {
        return Err("Enter an address or a search".into());
    }
    if text.contains("://") {
        let url = Url::parse(text).map_err(|_| "That address is not valid".to_string())?;
        return web_only(url);
    }
    if looks_like_other_scheme(text) {
        return Err("Only web addresses can be opened".into());
    }
    if text.contains(char::is_whitespace) || !(text.contains('.') || text.starts_with("localhost")) {
        return Url::parse_with_params("https://duckduckgo.com/", &[("q", text)]).map_err(|e| e.to_string());
    }
    let url = Url::parse(&format!("https://{text}")).map_err(|_| "That address is not valid".to_string())?;
    web_only(url)
}

fn web_only(url: Url) -> Result<Url, String> {
    match url.scheme() {
        "http" | "https" if url.host().is_some() => Ok(url),
        _ => Err("Only web addresses can be opened".into()),
    }
}

/// `javascript:alert(1)`, `mailto:x`, `tauri:`. A scheme followed by a digit is a port, as in `localhost:3000`.
fn looks_like_other_scheme(text: &str) -> bool {
    let Some((scheme, rest)) = text.split_once(':') else { return false };
    let scheme_ok = !scheme.is_empty()
        && scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c));
    scheme_ok && !rest.starts_with(|c: char| c.is_ascii_digit() || c.is_whitespace())
}

#[cfg(test)]
mod tests {
    use super::normalize_url;

    #[test]
    fn a_bare_host_gets_https() {
        assert_eq!(normalize_url("github.com").unwrap().as_str(), "https://github.com/");
        assert_eq!(normalize_url("  docs.rs/tauri  ").unwrap().as_str(), "https://docs.rs/tauri");
    }

    #[test]
    fn a_web_address_is_kept_as_written() {
        assert_eq!(normalize_url("https://example.com/a?b=1").unwrap().as_str(), "https://example.com/a?b=1");
        assert_eq!(normalize_url("http://127.0.0.1:8080/x").unwrap().host_str(), Some("127.0.0.1"));
    }

    #[test]
    fn localhost_with_a_port_is_an_address() {
        assert_eq!(normalize_url("localhost:3000").unwrap().as_str(), "https://localhost:3000/");
        assert_eq!(normalize_url("github.com:443").unwrap().host_str(), Some("github.com"));
    }

    #[test]
    fn other_schemes_are_refused() {
        for text in ["javascript:alert(1)", "file:///etc/passwd", "tauri://localhost", "about:blank", "mailto:a@b.c", "data:text/html,hi"] {
            assert!(normalize_url(text).is_err(), "{text} should be refused");
        }
    }

    #[test]
    fn text_that_is_not_an_address_is_searched() {
        let url = normalize_url("how do I rebase").unwrap();
        assert_eq!(url.host_str(), Some("duckduckgo.com"));
        assert!(url.query().unwrap().contains("q=how+do+I+rebase"));
    }

    #[test]
    fn empty_input_is_refused() {
        assert!(normalize_url("   ").is_err());
    }
}
