//! Addresses for the built-in browser. Whatever the user types becomes an http or https URL, or a search.

use tauri::Url;

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
