//! Optional user configuration: `~/.config/prjump/config.toml`.
//!
//! ```toml
//! # An ~/.ssh/config alias for a second GitHub account.
//! [hosts."github-work"]
//! web_host = "github.com"
//!
//! # A self-hosted GitHub Enterprise whose name says nothing.
//! [hosts."git.corp.example"]
//! provider = "github"
//! ```
//!
//! The file is looked up at `$PRJ_CONFIG`, then
//! `$XDG_CONFIG_HOME/prjump/config.toml`, then
//! `~/.config/prjump/config.toml`. A missing default file is not an error.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::Error;
use crate::provider::Provider;
use crate::remote::RemoteInfo;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    #[serde(default)]
    hosts: BTreeMap<String, HostEntry>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HostEntry {
    /// `github` or `bitbucket`, for hosts prj cannot recognise by name.
    pub provider: Option<String>,
    /// Host (optionally `host:port`) that serves the web pages.
    pub web_host: Option<String>,
}

#[derive(Debug, Default)]
pub struct Config {
    /// Keys are lowercased host names.
    hosts: BTreeMap<String, HostEntry>,
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        match config_path() {
            Some((path, explicit)) => Self::load_from(&path, explicit),
            None => Ok(Self::default()),
        }
    }

    fn load_from(path: &Path, must_exist: bool) -> Result<Self, Error> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !must_exist => {
                return Ok(Self::default());
            }
            Err(e) => {
                return Err(Error::user(format!("config {}: {e}", path.display())));
            }
        };
        Self::parse(&text).map_err(|msg| Error::user(format!("config {}: {msg}", path.display())))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let file: ConfigFile =
            toml::from_str(text).map_err(|e| e.to_string().trim().to_string())?;
        let mut hosts = BTreeMap::new();
        for (host, entry) in file.hosts {
            if let Some(provider) = &entry.provider {
                if !Provider::NAMES.contains(&provider.as_str()) {
                    return Err(format!(
                        "hosts.\"{host}\".provider must be one of {}, got '{provider}'",
                        Provider::NAMES.join(", ")
                    ));
                }
            }
            if let Some(web_host) = &entry.web_host {
                if split_web_host(web_host).is_none() {
                    return Err(format!(
                        "hosts.\"{host}\".web_host must be a host name with an optional :port, got '{web_host}'"
                    ));
                }
            }
            hosts.insert(host.to_lowercase(), entry);
        }
        Ok(Config { hosts })
    }

    /// Applies the entry for `remote.host`, if any. Returns the provider the
    /// entry names, which `--provider` still overrides.
    pub fn apply(&self, remote: &mut RemoteInfo) -> Option<String> {
        let entry = self.hosts.get(&remote.host.to_lowercase())?;
        if let Some((host, port)) = entry.web_host.as_deref().and_then(split_web_host) {
            remote.host = host.to_string();
            remote.port = port.map(str::to_string);
        }
        entry.provider.clone()
    }
}

/// `host` or `host:port`, where host is letters, digits, `-` and `.`.
fn split_web_host(value: &str) -> Option<(&str, Option<&str>)> {
    let (host, port) = match value.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (value, None),
    };
    let host_ok = !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.');
    let port_ok = port.is_none_or(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    (host_ok && port_ok).then_some((host, port))
}

/// The config path and whether the user named it explicitly.
fn config_path() -> Option<(PathBuf, bool)> {
    let env = |key: &str| std::env::var_os(key).filter(|v| !v.is_empty());
    if let Some(path) = env("PRJ_CONFIG") {
        return Some((PathBuf::from(path), true));
    }
    let base = env("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            env("HOME")
                .or_else(|| env("USERPROFILE"))
                .map(|home| PathBuf::from(home).join(".config"))
        })?;
    Some((base.join("prjump").join("config.toml"), false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_maps_to_web_host_and_provider() {
        let cfg = Config::parse(
            r#"
            [hosts."GitHub-Work"]
            web_host = "github.com"

            [hosts."git.corp.example"]
            provider = "github"
            web_host = "github.corp.example:8443"
            "#,
        )
        .unwrap();

        let mut r = RemoteInfo::new("github-work", "o", "r");
        assert_eq!(cfg.apply(&mut r), None);
        assert_eq!(r.authority(), "github.com");

        let mut r = RemoteInfo::new("git.corp.example", "o", "r");
        assert_eq!(cfg.apply(&mut r).as_deref(), Some("github"));
        assert_eq!(r.authority(), "github.corp.example:8443");

        let mut r = RemoteInfo::new("elsewhere", "o", "r");
        assert_eq!(cfg.apply(&mut r), None);
        assert_eq!(r.authority(), "elsewhere");
    }

    #[test]
    fn empty_file_is_fine() {
        assert!(Config::parse("").unwrap().hosts.is_empty());
    }

    #[test]
    fn bad_values_are_rejected_with_the_key() {
        let err = Config::parse("[hosts.x]\nprovider = \"gitlab\"\n").unwrap_err();
        assert!(err.contains("hosts.\"x\".provider"), "{err}");
        for bad in [
            "https://github.com",
            "github.com/o",
            "a b",
            "host:",
            "host:80x",
            "",
        ] {
            let text = format!("[hosts.x]\nweb_host = \"{bad}\"\n");
            assert!(Config::parse(&text).is_err(), "{bad}");
        }
    }

    #[test]
    fn unknown_keys_and_bad_toml_are_rejected() {
        assert!(Config::parse("[hosts.x]\nweb-host = \"a\"\n").is_err());
        assert!(Config::parse("colour = 1\n").is_err());
        assert!(Config::parse("[hosts.x\n").is_err());
    }

    #[test]
    fn explicit_missing_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.toml");
        assert!(Config::load_from(&path, true).is_err());
        assert!(Config::load_from(&path, false).unwrap().hosts.is_empty());
    }
}
