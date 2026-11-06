//! Opening the PR page in a browser.
//!
//! Python's `webbrowser.open` tries every `$BROWSER` entry first on every
//! platform and then falls back to the platform default. The `webbrowser`
//! crate reads `$BROWSER` on Linux/BSD but ignores it on macOS, so the
//! `$BROWSER` step is done here and the crate only provides the fallback.

use std::process::Command;

#[cfg(windows)]
const BROWSER_SEPARATOR: char = ';';
#[cfg(not(windows))]
const BROWSER_SEPARATOR: char = ':';

/// Returns true when some browser accepted the URL.
pub fn open(url: &str) -> bool {
    if let Some(browser_env) = std::env::var_os("BROWSER") {
        let browser_env = browser_env.to_string_lossy().into_owned();
        if try_browser_env(&browser_env, url) {
            return true;
        }
        // The entries failed; keep the crate from running them a second time.
        // SAFETY: prj is single-threaded and nothing reads the environment
        // concurrently at this point.
        unsafe { std::env::remove_var("BROWSER") };
    }
    webbrowser::open(url).is_ok()
}

/// Python `GenericBrowser`: each non-empty entry is a whole command name,
/// called with the URL as its only argument; exit status 0 is success.
fn try_browser_env(browser_env: &str, url: &str) -> bool {
    browser_env
        .split(BROWSER_SEPARATOR)
        .filter(|entry| !entry.is_empty())
        .any(|entry| {
            // stdin stays inherited: a text browser in $BROWSER needs the tty.
            Command::new(entry)
                .arg(url)
                .status()
                .is_ok_and(|status| status.success())
        })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn browser_env_tries_entries_in_order() {
        assert!(try_browser_env(
            "/nonexistent:/usr/bin/true",
            "https://example.com"
        ));
        assert!(!try_browser_env(
            "/nonexistent::/usr/bin/false",
            "https://example.com"
        ));
    }
}
