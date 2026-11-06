//! `--copy`: put the URL on the clipboard of the terminal in front of the
//! user with an OSC 52 escape sequence. This works over SSH, because the
//! sequence travels to the local terminal emulator, provided the terminal
//! allows OSC 52 writes.

use std::io::{IsTerminal, Write};

/// `ESC ] 52 ; c ; <base64> BEL`. Inside tmux the sequence is wrapped in a
/// DCS passthrough (`ESC P tmux; ... ESC \`, inner ESC doubled), which tmux
/// forwards when `allow-passthrough` is on.
pub fn osc52(text: &str, tmux: bool) -> String {
    let seq = format!("\x1b]52;c;{}\x07", base64(text.as_bytes()));
    if tmux {
        format!("\x1bPtmux;{}\x1b\\", seq.replace('\x1b', "\x1b\x1b"))
    } else {
        seq
    }
}

/// Writes the sequence to the controlling terminal (`/dev/tty`), or to
/// stderr when that is a terminal. stdout is left alone so `--print` output
/// stays clean in pipes.
pub fn copy(text: &str) -> Result<(), String> {
    let tmux = std::env::var_os("TMUX").is_some_and(|v| !v.is_empty());
    let seq = osc52(text, tmux);

    #[cfg(unix)]
    if let Ok(mut tty) = std::fs::OpenOptions::new().write(true).open("/dev/tty") {
        return tty.write_all(seq.as_bytes()).map_err(|e| e.to_string());
    }

    let mut stderr = std::io::stderr();
    if stderr.is_terminal() {
        return stderr.write_all(seq.as_bytes()).map_err(|e| e.to_string());
    }
    Err("no terminal to send the clipboard sequence to".to_string())
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        for (input, want) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), want, "{input}");
        }
    }

    #[test]
    fn plain_sequence() {
        assert_eq!(osc52("foo", false), "\x1b]52;c;Zm9v\x07");
    }

    #[test]
    fn tmux_passthrough_doubles_escapes() {
        assert_eq!(osc52("foo", true), "\x1bPtmux;\x1b\x1b]52;c;Zm9v\x07\x1b\\");
    }
}
