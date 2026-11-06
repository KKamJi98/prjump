# Roadmap

Ideas that are not in prj 0.3.0. Nothing here is scheduled.

## P2

- **GitLab.** `/-/merge_requests/new?merge_request[source_branch]=...&merge_request[target_branch]=...`,
  including nested groups (`group/sub/repo`).
- **Bitbucket Server / Data Center.** `/projects/<KEY>/repos/<repo>/pull-requests?create&sourceBranch=...&targetBranch=...`;
  the remote path (`scm/<key>/<repo>`) differs from Bitbucket Cloud.
- **`--json`.** Print the resolved remote, provider, source, target and URL
  for scripts.
- **Title and body.** Fill GitHub's `title` and `body` query parameters from
  the branch's last commit (opt-in; long bodies hit URL length limits).

## P3

- **Existing pull requests.** When a PR for the branch is already open, go
  to it instead of the compare page (`gh pr view --web` when `gh` is
  installed, or the provider API).
- **`prj` with no subcommand runs `open`.** Today it prints help and exits 1,
  as 0.2.x did. Changing that is a 1.0 decision.

## Known gaps

- Follow custom push refspecs when naming the head branch.
- Cross-fork heads for forks with a different repository name
  (`owner:repo:branch`), once confirmed against GitHub.
