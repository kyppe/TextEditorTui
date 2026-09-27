//! Opening a link in the desktop's browser, for `gx`.
//!
//! Shells out to the platform's "open this" helper rather than linking a
//! crate, the same approach `clipboard.rs` takes. The URL is passed as a
//! single argument to `Command`, never through a shell, so nothing in it can
//! be interpreted as a command.

use std::process::{Command, Stdio};

/// Openers to try in order. Each takes the URL as its only argument.
const OPENERS: &[&str] = &["xdg-open", "open", "gio", "wslview"];

/// Schemes we are willing to hand to the desktop. A journal is just a JSON
/// file that could have come from anywhere, so this refuses anything more
/// exotic than a web or mail link rather than launching an arbitrary
/// protocol handler.
const ALLOWED: &[&str] = &["http://", "https://", "mailto:"];

/// Normalises a link target into something openable, or explains why not.
///
/// A bare `example.com` or `www.example.com` gets `https://`, which is what
/// people expect when they paste a domain into a note.
pub fn normalize(target: &str) -> Result<String, String> {
    let t = target.trim();
    if t.is_empty() {
        return Err("No URL given".into());
    }
    if t.contains(char::is_whitespace) {
        return Err("A URL can't contain spaces".into());
    }
    if ALLOWED.iter().any(|s| t.starts_with(s)) {
        return Ok(t.to_string());
    }
    // An unknown but explicit scheme is refused rather than https-prefixed:
    // silently turning `javascript:…` into a web address would be worse than
    // saying no. The `//` check keeps `example.com/a:b` out of this branch.
    if let Some(colon) = t.find(':') {
        let looks_schemeish = t[..colon]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.');
        if looks_schemeish && !t[..colon].contains('/') {
            return Err(format!(
                "Only http, https and mailto links can be opened (got \"{}:\")",
                &t[..colon]
            ));
        }
    }
    Ok(format!("https://{t}"))
}

/// Hands `url` to the desktop. Returns the normalised URL that was opened.
pub fn open(target: &str) -> Result<String, String> {
    let url = normalize(target)?;
    for opener in OPENERS {
        let spawned = Command::new(opener)
            .arg(&url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if spawned.is_ok() {
            // Deliberately not waited on: the browser outlives us, and
            // blocking here would freeze the editor until it exits.
            return Ok(url);
        }
    }
    Err("No opener found (install xdg-utils for xdg-open)".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_domains_become_https() {
        assert_eq!(normalize("example.com").unwrap(), "https://example.com");
        assert_eq!(
            normalize("www.example.com/a?b=1").unwrap(),
            "https://www.example.com/a?b=1"
        );
    }

    #[test]
    fn web_and_mail_links_pass_through_untouched() {
        for url in [
            "http://example.com",
            "https://example.com/x#y",
            "mailto:someone@example.com",
        ] {
            assert_eq!(normalize(url).unwrap(), url);
        }
    }

    /// A journal file could come from anywhere, so an unexpected scheme is
    /// refused instead of being launched or quietly rewritten.
    #[test]
    fn other_schemes_are_refused() {
        for bad in ["javascript:alert(1)", "file:///etc/passwd", "ftp://x.test"] {
            assert!(normalize(bad).is_err(), "{bad} should be refused");
        }
    }

    #[test]
    fn empty_and_spaced_targets_are_refused() {
        assert!(normalize("   ").is_err());
        assert!(normalize("two words").is_err());
    }
}
