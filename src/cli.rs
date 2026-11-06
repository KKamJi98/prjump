use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::{ArgAction, Args, CommandFactory, Parser, Subcommand};

use crate::browser;
use crate::config::Config;
use crate::error::Error;
use crate::git::{Git, SystemGit, get_current_branch, get_default_branch, get_remote_url};
use crate::head::resolve_head;
use crate::provider::{Provider, resolve_provider};
use crate::remote::{RemoteInfo, parse_remote_url};

// `args_override_self` and `infer_long_args` keep two argparse behaviours
// that shell aliases rely on: a repeated option takes the last value, and a
// unique prefix of a long option (`--tar`) is accepted.
#[derive(Parser, Debug)]
#[command(
    name = "prj",
    version,
    about = "Open GitHub/Bitbucket Cloud pull request pages.",
    disable_version_flag = true,
    args_override_self = true,
    infer_long_args = true
)]
pub struct Cli {
    /// Show program's version number and exit
    #[arg(long, action = ArgAction::Version)]
    version: Option<bool>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Open a pull request page
    #[command(args_override_self = true, infer_long_args = true)]
    Open(OpenArgs),
}

#[derive(Args, Debug, Clone, Default)]
pub struct OpenArgs {
    // `allow_negative_numbers`: argparse takes `-t -1` as the value "-1".
    /// Git remote name
    #[arg(long, default_value = "origin", allow_negative_numbers = true)]
    pub remote: String,

    /// Target branch to merge into (GitHub base branch)
    #[arg(short = 't', long, allow_negative_numbers = true)]
    pub target: Option<String>,

    /// Source branch to merge from (defaults to current branch)
    #[arg(short = 's', long, allow_negative_numbers = true)]
    pub source: Option<String>,

    /// Override provider detection
    #[arg(long, value_parser = Provider::NAMES)]
    pub provider: Option<String>,

    /// Print the URL instead of opening the browser
    #[arg(long)]
    pub print: bool,
}

pub fn parse_args<I, T>(argv: I) -> Result<Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    Cli::try_parse_from(argv)
}

/// Process entry point. Every failure ends with exit status 1 except
/// argument errors, which keep clap's (and argparse's) status 2.
pub fn main() -> ExitCode {
    install_panic_hook();

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let _ = err.print();
            return ExitCode::from(u8::try_from(err.exit_code()).unwrap_or(2));
        }
    };

    let Some(command) = cli.command else {
        // argparse printed the full help to stdout and exited 1 here.
        let _ = Cli::command().print_help();
        return ExitCode::from(1);
    };

    let result = match command {
        Command::Open(args) => cmd_open(&args, &SystemGit),
    };
    match result {
        Ok(code) => code,
        Err(err) => {
            let _ = writeln!(io::stderr(), "{err}");
            ExitCode::from(1)
        }
    }
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload();
        let msg = payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "internal error".to_string());
        let _ = writeln!(io::stderr(), "{}", Error::unexpected(msg));
        std::process::exit(1);
    }));
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().filter(|v| !v.is_empty())
}

pub fn cmd_open(args: &OpenArgs, git: &dyn Git) -> Result<ExitCode, Error> {
    let config = Config::load()?;
    let remote_url = get_remote_url(git, &args.remote)?;
    let mut remote = parse_remote_url(&remote_url)?;
    let config_provider = config.apply(&mut remote);
    let provider_choice = args.provider.as_deref().or(config_provider.as_deref());
    let provider = resolve_provider(&remote, provider_choice)?;

    let mut warnings = Vec::new();
    let mut head_owner = None;
    let head = match non_empty(&args.source) {
        Some(source) => source.to_string(),
        None => {
            let local = get_current_branch(git)?;
            let head = resolve_head(git, &local, &args.remote)?;
            warnings.extend(head.warnings);
            let fork_remote = head.remote.as_deref().filter(|r| *r != args.remote);
            if let Some(owner) = fork_remote.and_then(|r| fork_owner(git, &config, r, &remote)) {
                match provider {
                    Provider::GitHub => head_owner = Some(owner),
                    Provider::Bitbucket => warnings.push(format!(
                        "warning: '{}' is pushed to {owner}'s fork; choose that repository as the source on the Bitbucket page",
                        head.branch
                    )),
                }
            }
            head.branch
        }
    };

    let base = match provider {
        Provider::GitHub => Some(match non_empty(&args.target) {
            Some(target) => target.to_string(),
            None => get_default_branch(git, &args.remote)?,
        }),
        Provider::Bitbucket => args.target.clone(),
    };

    let url = provider.build_pr_url_from(&remote, base.as_deref(), &head, head_owner.as_deref())?;
    for warning in &warnings {
        let _ = writeln!(io::stderr(), "{warning}");
    }

    if args.print {
        print_line(&url)?;
        return Ok(ExitCode::SUCCESS);
    }

    if !browser::open(&url) {
        let _ = writeln!(io::stderr(), "Failed to open browser. URL: {url}");
        return Ok(ExitCode::from(1));
    }
    Ok(ExitCode::SUCCESS)
}

/// The owner of `head_remote` when it is another fork of the same repository
/// host as the target. Any problem reading it just means "not a fork".
fn fork_owner(
    git: &dyn Git,
    config: &Config,
    head_remote: &str,
    target: &RemoteInfo,
) -> Option<String> {
    let url = get_remote_url(git, head_remote).ok()?;
    let mut head = parse_remote_url(&url).ok()?;
    config.apply(&mut head);
    let same_host = head.authority().eq_ignore_ascii_case(&target.authority());
    (same_host && !head.owner.eq_ignore_ascii_case(&target.owner)).then_some(head.owner)
}

/// Writes one line to stdout. A closed pipe (`prj open --print | true`) is
/// not an error; `println!` would panic there.
fn print_line(line: &str) -> Result<(), Error> {
    let mut stdout = io::stdout().lock();
    match writeln!(stdout, "{line}").and_then(|()| stdout.flush()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(Error::unexpected(format!("failed to write to stdout: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_args(argv: &[&str]) -> OpenArgs {
        let cli = parse_args(std::iter::once("prj").chain(argv.iter().copied())).unwrap();
        match cli.command {
            Some(Command::Open(args)) => args,
            None => panic!("no subcommand"),
        }
    }

    // Ported from tests/test_cli.py (prjump 0.2.2).

    #[test]
    fn open_short_flags_map_to_target_and_source() {
        let args = open_args(&["open", "-t", "main", "-s", "feature/x"]);
        assert_eq!(args.target.as_deref(), Some("main"));
        assert_eq!(args.source.as_deref(), Some("feature/x"));
    }

    #[test]
    fn open_long_flags_work() {
        let args = open_args(&["open", "--target", "develop", "--source", "topic"]);
        assert_eq!(args.target.as_deref(), Some("develop"));
        assert_eq!(args.source.as_deref(), Some("topic"));
    }

    #[test]
    fn open_help_flags_exit_zero() {
        for flag in ["-h", "--help"] {
            let err = parse_args(["prj", "open", flag]).unwrap_err();
            assert_eq!(err.exit_code(), 0, "{flag}");
        }
    }

    // argparse compatibility settings.

    #[test]
    fn repeated_options_take_the_last_value() {
        let args = open_args(&["open", "-t", "a", "-t", "b", "--print", "--print"]);
        assert_eq!(args.target.as_deref(), Some("b"));
        assert!(args.print);
    }

    #[test]
    fn unique_long_prefixes_are_accepted() {
        let args = open_args(&["open", "--tar", "dev", "--pri", "--pro", "github"]);
        assert_eq!(args.target.as_deref(), Some("dev"));
        assert!(args.print);
        assert_eq!(args.provider.as_deref(), Some("github"));
    }

    #[test]
    fn ambiguous_prefix_and_short_version_are_rejected() {
        assert_eq!(
            parse_args(["prj", "open", "--p", "x"])
                .unwrap_err()
                .exit_code(),
            2
        );
        assert_eq!(parse_args(["prj", "-V"]).unwrap_err().exit_code(), 2);
    }

    #[test]
    fn provider_choices_are_case_sensitive() {
        assert_eq!(
            parse_args(["prj", "open", "--provider", "GitHub"])
                .unwrap_err()
                .exit_code(),
            2
        );
    }
}
