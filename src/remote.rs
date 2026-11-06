//! Remote URL parsing.
//!
//! The `scheme://` branch mirrors CPython 3.13 `urllib.parse.urlparse` as
//! prjump 0.2.2 used it. The `url` crate is deliberately not used: it rejects
//! scp-style remotes, normalises `..`, percent-encodes paths, keeps brackets
//! around IPv6 hosts and keeps the case of non-special hosts, all of which
//! change the URL prjump builds.

use std::net::{Ipv4Addr, Ipv6Addr};

use crate::error::Error;
use crate::pystr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteInfo {
    pub host: String,
    pub owner: String,
    pub repo: String,
    pub scheme: String,
    /// Web port from an `http(s)://host:port/...` remote. SSH ports and the
    /// scheme's default port are not kept.
    pub port: Option<String>,
}

impl RemoteInfo {
    pub fn new(host: &str, owner: &str, repo: &str) -> Self {
        RemoteInfo {
            host: host.to_string(),
            owner: owner.to_string(),
            repo: repo.to_string(),
            scheme: "https".to_string(),
            port: None,
        }
    }

    /// `host[:port]` for the web URL, with brackets around IPv6 hosts.
    pub fn authority(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        match &self.port {
            Some(port) => format!("{host}:{port}"),
            None => host,
        }
    }
}

pub fn parse_remote_url(url: &str) -> Result<RemoteInfo, Error> {
    let url = pystr::strip(url);
    let mut scheme = "https".to_string();
    let mut port = None;

    let (host, path) = if url.contains("://") {
        let parsed = urlparse(url)?;
        if parsed.scheme == "http" || parsed.scheme == "https" {
            scheme = parsed.scheme.clone();
            let default = if scheme == "http" { "80" } else { "443" };
            port = parsed
                .port()
                .filter(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) && *p != default)
                .map(str::to_string);
        }
        (parsed.hostname().unwrap_or_default(), parsed.path)
    } else {
        let Some((user_host, path)) = url.split_once(':') else {
            return Err(Error::user("Unsupported remote URL format."));
        };
        // Python: user_host.split("@", 1)[-1]
        let host = user_host.split_once('@').map_or(user_host, |(_, h)| h);
        (host.to_string(), path.to_string())
    };

    if host.is_empty() {
        return Err(Error::user("Could not parse host from remote URL."));
    }

    let mut path = path.trim_start_matches('/');
    if let Some(stripped) = path.strip_suffix(".git") {
        path = stripped;
    }

    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() < 2 {
        return Err(Error::user(
            "Remote URL does not look like a hosted git repository.",
        ));
    }

    let (repo, owner) = parts.split_last().expect("len checked above");
    Ok(RemoteInfo {
        host,
        owner: owner.join("/"),
        repo: (*repo).to_string(),
        scheme,
        port,
    })
}

#[derive(Debug)]
struct ParsedUrl {
    scheme: String,
    netloc: String,
    path: String,
}

impl ParsedUrl {
    /// The port part of the netloc, as Python `_hostinfo` splits it.
    fn port(&self) -> Option<&str> {
        let hostinfo = rpartition_after(&self.netloc, '@');
        let port = match hostinfo.split_once('[') {
            Some((_, bracketed)) => bracketed
                .split_once(']')
                .and_then(|(_, rest)| rest.split_once(':'))
                .map(|(_, p)| p),
            None => hostinfo.split_once(':').map(|(_, p)| p),
        };
        port.filter(|p| !p.is_empty())
    }

    /// Python `ParseResult.hostname`: lowercased host without brackets or
    /// port. A scoped IPv6 zone keeps its case.
    fn hostname(&self) -> Option<String> {
        let hostinfo = rpartition_after(&self.netloc, '@');
        let hostname = match hostinfo.split_once('[') {
            Some((_, bracketed)) => bracketed.split_once(']').map_or(bracketed, |(h, _)| h),
            None => hostinfo.split_once(':').map_or(hostinfo, |(h, _)| h),
        };
        if hostname.is_empty() {
            return None;
        }
        Some(match hostname.split_once('%') {
            Some((h, zone)) => format!("{}%{}", h.to_lowercase(), zone),
            None => hostname.to_lowercase(),
        })
    }
}

/// Schemes for which `urlparse` splits `;params` off the path.
const USES_PARAMS: &[&str] = &[
    "", "ftp", "hdl", "prospero", "http", "imap", "https", "shttp", "rtsp", "rtsps", "rtspu",
    "sip", "sips", "mms", "sftp", "tel",
];

fn urlparse(input: &str) -> Result<ParsedUrl, Error> {
    // urlsplit: strip leading C0 control/space, drop tab/CR/LF anywhere.
    let mut url: String = input
        .trim_start_matches(|c: char| c <= '\u{20}')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\r' | '\n'))
        .collect();

    let mut scheme = String::new();
    if let Some(i) = url.find(':') {
        let first = url.chars().next();
        if i > 0
            && first.is_some_and(|c| c.is_ascii_alphabetic())
            && url[..i]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        {
            scheme = url[..i].to_ascii_lowercase();
            url = url[i + 1..].to_string();
        }
    }

    let mut netloc = String::new();
    if let Some(rest) = url.strip_prefix("//") {
        let delim = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        netloc = rest[..delim].to_string();
        url = rest[delim..].to_string();
        let open = netloc.contains('[');
        let close = netloc.contains(']');
        if open != close {
            return Err(Error::unexpected("Invalid IPv6 URL"));
        }
        if open && close {
            check_bracketed_netloc(&netloc)?;
        }
    }

    if let Some((before, _fragment)) = url.split_once('#') {
        url = before.to_string();
    }
    if let Some((before, _query)) = url.split_once('?') {
        url = before.to_string();
    }

    if USES_PARAMS.contains(&scheme.as_str()) && url.contains(';') {
        url = split_params(&url).to_string();
    }

    Ok(ParsedUrl {
        scheme,
        netloc,
        path: url,
    })
}

/// Python `_splitparams`, returning only the part before the params.
fn split_params(url: &str) -> &str {
    match url.rfind('/') {
        Some(slash) => match url[slash..].find(';') {
            Some(j) => &url[..slash + j],
            None => url,
        },
        None => &url[..url.find(';').expect("caller checked for ';'")],
    }
}

/// Python `s.rpartition(sep)[2]`.
fn rpartition_after(s: &str, sep: char) -> &str {
    s.rsplit_once(sep).map_or(s, |(_, after)| after)
}

fn check_bracketed_netloc(netloc: &str) -> Result<(), Error> {
    let host_and_port = rpartition_after(netloc, '@');
    let hostname = match host_and_port.split_once('[') {
        Some((before, bracketed)) => {
            if !before.is_empty() {
                return Err(Error::unexpected("Invalid IPv6 URL"));
            }
            let (hostname, port) = bracketed.split_once(']').unwrap_or((bracketed, ""));
            if !port.is_empty() && !port.starts_with(':') {
                return Err(Error::unexpected("Invalid IPv6 URL"));
            }
            hostname
        }
        None => host_and_port
            .split_once(':')
            .map_or(host_and_port, |(h, _)| h),
    };
    check_bracketed_host(hostname)
}

fn check_bracketed_host(hostname: &str) -> Result<(), Error> {
    if let Some(rest) = hostname.strip_prefix('v') {
        // \Av[a-fA-F0-9]+\..+\Z
        let hex_len = rest.chars().take_while(|c| c.is_ascii_hexdigit()).count();
        let valid = hex_len > 0
            && rest[hex_len..]
                .strip_prefix('.')
                .is_some_and(|tail| !tail.is_empty());
        if !valid {
            return Err(Error::unexpected("IPvFuture address is invalid"));
        }
        return Ok(());
    }
    if hostname.parse::<Ipv4Addr>().is_ok() {
        return Err(Error::unexpected("An IPv4 address cannot be in brackets"));
    }
    let (addr, scope) = match hostname.split_once('%') {
        Some((addr, scope)) => (addr, Some(scope)),
        None => (hostname, None),
    };
    let scope_ok = scope.is_none_or(|s| !s.is_empty() && !s.contains('%'));
    if scope_ok && addr.parse::<Ipv6Addr>().is_ok() {
        return Ok(());
    }
    Err(Error::unexpected(format!(
        "'{hostname}' does not appear to be an IPv4 or IPv6 address"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err_msg(url: &str) -> String {
        parse_remote_url(url).unwrap_err().message().to_string()
    }

    // Ported from tests/test_cli.py (prjump 0.2.2).

    #[test]
    fn parse_remote_url_ssh_format() {
        let r = parse_remote_url("git@github.com:octocat/Hello-World.git").unwrap();
        assert_eq!(r.host, "github.com");
        assert_eq!(r.owner, "octocat");
        assert_eq!(r.repo, "Hello-World");
    }

    #[test]
    fn parse_remote_url_https_format() {
        let r = parse_remote_url("https://github.com/octocat/Hello-World.git").unwrap();
        assert_eq!(r.host, "github.com");
        assert_eq!(r.owner, "octocat");
        assert_eq!(r.repo, "Hello-World");
        assert_eq!(r.scheme, "https");
    }

    #[test]
    fn parse_remote_url_http_scheme_preserved() {
        let r = parse_remote_url("http://github.example.com/org/repo").unwrap();
        assert_eq!(r.scheme, "http");
    }

    #[test]
    fn parse_remote_url_git_protocol() {
        let r = parse_remote_url("git://github.com/octocat/repo.git").unwrap();
        assert_eq!(r.host, "github.com");
        assert_eq!(r.repo, "repo");
    }

    #[test]
    fn parse_remote_url_removes_dot_git_suffix() {
        let r = parse_remote_url("https://github.com/org/myrepo.git").unwrap();
        assert_eq!(r.repo, "myrepo");
    }

    #[test]
    fn parse_remote_url_no_owner_raises() {
        assert!(
            err_msg("https://github.com/single-segment")
                .contains("does not look like a hosted git repository")
        );
    }

    #[test]
    fn parse_remote_url_ssh_no_colon_raises() {
        assert!(matches!(
            parse_remote_url("git@github.com/no-colon/repo.git"),
            Err(Error::User(_))
        ));
    }

    #[test]
    fn parse_remote_url_bitbucket_ssh() {
        let r = parse_remote_url("git@bitbucket.org:atlassian/stash.git").unwrap();
        assert_eq!(r.host, "bitbucket.org");
        assert_eq!(r.owner, "atlassian");
        assert_eq!(r.repo, "stash");
    }

    // urlparse corner cases checked against CPython 3.13.

    #[test]
    fn hostname_is_lowercased_only_for_scheme_urls() {
        assert_eq!(
            parse_remote_url("https://GitHub.COM/O/R").unwrap().host,
            "github.com"
        );
        assert_eq!(
            parse_remote_url("git@GitHub.com:O/R").unwrap().host,
            "GitHub.com"
        );
    }

    #[test]
    fn credentials_port_query_and_fragment_are_dropped() {
        let r = parse_remote_url("https://u:p@github.com:8443/o/r.git?x=1#f").unwrap();
        assert_eq!(
            (r.host.as_str(), r.owner.as_str(), r.repo.as_str()),
            ("github.com", "o", "r")
        );
    }

    #[test]
    fn ipv6_hosts_lose_brackets() {
        assert_eq!(
            parse_remote_url("https://[::1]:3000/o/r").unwrap().host,
            "::1"
        );
    }

    #[test]
    fn invalid_ipv6_is_an_unexpected_error() {
        assert_eq!(
            parse_remote_url("https://[::1/o/r").unwrap_err(),
            Error::unexpected("Invalid IPv6 URL")
        );
        assert_eq!(
            parse_remote_url("https://[1.2.3.4]/o/r").unwrap_err(),
            Error::unexpected("An IPv4 address cannot be in brackets")
        );
        assert_eq!(
            parse_remote_url("https://[abc]/o/r").unwrap_err(),
            Error::unexpected("'abc' does not appear to be an IPv4 or IPv6 address")
        );
    }

    #[test]
    fn params_are_split_only_for_param_schemes() {
        assert_eq!(
            parse_remote_url("https://github.com/o/r;x").unwrap().repo,
            "r"
        );
        assert_eq!(
            parse_remote_url("ssh://git@github.com/o/r;x").unwrap().repo,
            "r;x"
        );
    }

    #[test]
    fn path_is_not_normalised() {
        let r = parse_remote_url("https://github.com/org/../repo/x").unwrap();
        assert_eq!((r.owner.as_str(), r.repo.as_str()), ("org/../repo", "x"));
    }

    #[test]
    fn invalid_scheme_leaves_no_host() {
        assert_eq!(
            err_msg("1http://github.com/o/r"),
            "Could not parse host from remote URL."
        );
        assert_eq!(
            err_msg("file:///srv/repo.git"),
            "Could not parse host from remote URL."
        );
    }
}
