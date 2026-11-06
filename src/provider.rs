use crate::error::Error;
use crate::pystr::quote;
use crate::remote::RemoteInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    GitHub,
    Bitbucket,
}

impl Provider {
    pub const NAMES: [&'static str; 2] = ["bitbucket", "github"];

    pub fn name(self) -> &'static str {
        match self {
            Provider::GitHub => "github",
            Provider::Bitbucket => "bitbucket",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "github" => Some(Provider::GitHub),
            "bitbucket" => Some(Provider::Bitbucket),
            _ => None,
        }
    }

    /// Build the "new pull request" URL. Empty branch names count as absent,
    /// as they did in Python.
    pub fn build_pr_url(
        self,
        remote: &RemoteInfo,
        base: Option<&str>,
        head: &str,
    ) -> Result<String, Error> {
        let base = base.filter(|b| !b.is_empty());
        let prefix = format!(
            "{}://{}/{}/{}",
            remote.scheme, remote.host, remote.owner, remote.repo
        );
        match self {
            Provider::GitHub => {
                let Some(base) = base else {
                    return Err(Error::user(
                        "GitHub requires a target branch. Use --target to specify one.",
                    ));
                };
                Ok(format!(
                    "{prefix}/compare/{}...{}",
                    quote(base),
                    quote(head)
                ))
            }
            Provider::Bitbucket => {
                let mut query = format!("source={}", quote(head));
                if let Some(base) = base {
                    query.push_str(&format!("&dest={}", quote(base)));
                }
                query.push_str("&t=1");
                Ok(format!("{prefix}/pull-requests/new?{query}"))
            }
        }
    }
}

pub fn resolve_provider(remote: &RemoteInfo, override_: Option<&str>) -> Result<Provider, Error> {
    let key = match override_.filter(|o| !o.is_empty()) {
        Some(o) => o.to_lowercase(),
        None => {
            let host = remote.host.to_lowercase();
            if host.ends_with("github.com") {
                "github".to_string()
            } else if host.ends_with("bitbucket.org") {
                "bitbucket".to_string()
            } else {
                return Err(Error::user(
                    "Unsupported host. Use --provider github|bitbucket to override.",
                ));
            }
        }
    };
    Provider::from_name(&key)
        .ok_or_else(|| Error::user("Unsupported provider. Use github or bitbucket."))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ported from tests/test_cli.py (prjump 0.2.2).

    #[test]
    fn resolve_provider_github_com() {
        let r = RemoteInfo::new("github.com", "org", "repo");
        assert_eq!(resolve_provider(&r, None).unwrap().name(), "github");
    }

    #[test]
    fn resolve_provider_bitbucket_org() {
        let r = RemoteInfo::new("bitbucket.org", "org", "repo");
        assert_eq!(resolve_provider(&r, None).unwrap().name(), "bitbucket");
    }

    #[test]
    fn resolve_provider_override_forces_provider() {
        let r = RemoteInfo::new("github.example.internal", "org", "repo");
        assert_eq!(
            resolve_provider(&r, Some("github")).unwrap().name(),
            "github"
        );
    }

    #[test]
    fn resolve_provider_unknown_host_raises() {
        let r = RemoteInfo::new("gitlab.com", "org", "repo");
        let err = resolve_provider(&r, None).unwrap_err();
        assert!(err.message().contains("Unsupported host"));
    }

    #[test]
    fn resolve_provider_invalid_override_raises() {
        let r = RemoteInfo::new("github.com", "org", "repo");
        let err = resolve_provider(&r, Some("gitlab")).unwrap_err();
        assert!(err.message().contains("Unsupported provider"));
    }

    #[test]
    fn resolve_provider_subdomain_github_enterprise() {
        let r = RemoteInfo::new("github.mycompany.github.com", "org", "repo");
        assert_eq!(resolve_provider(&r, None).unwrap().name(), "github");
    }

    #[test]
    fn github_url_encodes_branches() {
        let r = RemoteInfo::new("github.com", "o", "r");
        assert_eq!(
            Provider::GitHub
                .build_pr_url(&r, Some("main"), "feature/x y")
                .unwrap(),
            "https://github.com/o/r/compare/main...feature%2Fx%20y"
        );
        assert!(Provider::GitHub.build_pr_url(&r, Some(""), "x").is_err());
    }

    #[test]
    fn bitbucket_url_dest_is_optional() {
        let r = RemoteInfo::new("bitbucket.org", "o", "r");
        assert_eq!(
            Provider::Bitbucket.build_pr_url(&r, None, "f/x").unwrap(),
            "https://bitbucket.org/o/r/pull-requests/new?source=f%2Fx&t=1"
        );
        assert_eq!(
            Provider::Bitbucket
                .build_pr_url(&r, Some("main"), "f")
                .unwrap(),
            "https://bitbucket.org/o/r/pull-requests/new?source=f&dest=main&t=1"
        );
    }
}
