//! Small helpers that reproduce Python string semantics used by prjump 0.2.2.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

/// Characters `urllib.parse.quote(s, safe="")` leaves alone: ASCII letters,
/// digits and `_.-~`. `NON_ALPHANUMERIC` alone would also encode `_.-~`.
const PY_QUOTE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Same output as Python `urllib.parse.quote(s, safe="")` for `str` input.
pub fn quote(s: &str) -> String {
    utf8_percent_encode(s, PY_QUOTE).to_string()
}

/// Python `str.isspace()`: Unicode White_Space plus the ASCII separators
/// U+001C..U+001F, which Python also treats as whitespace.
fn is_py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Python `str.strip()` with no arguments.
pub fn strip(s: &str) -> &str {
    s.trim_matches(is_py_space)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_matches_python_safe_empty() {
        assert_eq!(quote("feature/x y"), "feature%2Fx%20y");
        assert_eq!(quote("a-b_c.d~e"), "a-b_c.d~e");
        assert_eq!(quote("한"), "%ED%95%9C");
        assert_eq!(quote("a+b&c=d?e#f%"), "a%2Bb%26c%3Dd%3Fe%23f%25");
    }

    #[test]
    fn strip_matches_python_isspace() {
        assert_eq!(strip("\u{1c} main \n"), "main");
        assert_eq!(strip("\u{a0}x\u{2003}"), "x");
    }
}
