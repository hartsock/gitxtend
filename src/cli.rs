//! The `gitxtend` command line — one implementation, two front ends.
//!
//! [`run`] is the whole CLI as a pure function: argv in, exit code and captured
//! streams out. It touches no process globals, so the `gitxtend` binary
//! (`src/main.rs`) and the Python console script (`gitxtend._cli`, via the
//! `cli_main` PyO3 wrapper) are both thin shims over *this* code rather than two
//! parsers that agree only by review. Nothing to keep in sync, so nothing to
//! drift.
//!
//! Argument parsing is hand-rolled rather than pulling `clap`: the crate has two
//! runtime dependencies on purpose, and this surface is two subcommands. Being a
//! pure function over `&[String]` it is tested directly, without spawning
//! anything.

use std::path::{Path, PathBuf};

use crate::repo::{
    submodule_status, update_submodules, SubmoduleChange, SubmoduleStatusEntry, UpdateOptions,
};

/// Exit code, plus whatever the command would have written to each stream.
///
/// Returned rather than printed so the same code can serve a binary that prints
/// to the real streams and a Python wrapper that forwards them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliOutcome {
    /// `0` success, `1` a git operation failed, `2` bad usage.
    pub code: u8,
    pub stdout: String,
    pub stderr: String,
}

impl CliOutcome {
    fn ok(stdout: String) -> Self {
        Self {
            code: 0,
            stdout,
            stderr: String::new(),
        }
    }
}

const USAGE: &str = "\
gitxtend — gitoxide-backed git repository tending

USAGE:
    gitxtend submodule sync   [DIR] [OPTIONS]
    gitxtend submodule status [DIR] [OPTIONS]

`submodule sync` is the one-command form of \"keep every submodule on the tip of
the branch it tracks\": it updates each submodule, prints what moved, and with
--commit records the bumps in the superproject so the update survives the next
checkout. DIR defaults to the current directory.

OPTIONS (sync):
    -c, --commit          Record the moved gitlinks as a superproject commit.
    -m, --message MSG     Commit message to use with --commit.
        --no-remote       Restore submodules to the recorded SHAs instead of
                          advancing them to their tracked branch tips.
        --no-recursive    Do not recurse into nested submodules.
        --json            Emit machine-readable JSON.

OPTIONS (status):
        --no-recursive    Do not recurse into nested submodules.
        --json            Emit machine-readable JSON.

GLOBAL:
    -h, --help            Print this help.
    -V, --version         Print the version.

EXIT CODES:
    0  success
    1  a git operation failed (details on stderr)
    2  bad usage
";

#[derive(Debug, PartialEq, Eq)]
enum Cmd {
    Help,
    Version,
    Status {
        dir: PathBuf,
        recursive: bool,
        json: bool,
    },
    Sync {
        dir: PathBuf,
        recursive: bool,
        remote: bool,
        commit: bool,
        message: Option<String>,
        json: bool,
    },
}

/// Parse argv (without the program name) into a command.
///
/// Returns `Err(message)` for a usage error; the caller prints it and exits 2.
fn parse_args(args: &[String]) -> Result<Cmd, String> {
    let mut it = args.iter().map(String::as_str);

    let first = match it.next() {
        None => return Ok(Cmd::Help),
        Some(a) => a,
    };
    match first {
        "-h" | "--help" | "help" => return Ok(Cmd::Help),
        "-V" | "--version" | "version" => return Ok(Cmd::Version),
        "submodule" | "submodules" => {}
        other => return Err(format!("unknown command `{other}`")),
    }

    let sub = it
        .next()
        .ok_or_else(|| "`submodule` needs a subcommand: `sync` or `status`".to_string())?;
    let syncing = match sub {
        "sync" | "update" => true,
        "status" => false,
        "-h" | "--help" => return Ok(Cmd::Help),
        other => return Err(format!("unknown `submodule` subcommand `{other}`")),
    };

    let mut dir: Option<PathBuf> = None;
    let mut recursive = true;
    let mut remote = true;
    let mut commit = false;
    let mut message: Option<String> = None;
    let mut json = false;

    while let Some(arg) = it.next() {
        match arg {
            "-h" | "--help" => return Ok(Cmd::Help),
            "--no-recursive" => recursive = false,
            "--recursive" => recursive = true,
            "--json" => json = true,
            "--no-remote" if syncing => remote = false,
            "--remote" if syncing => remote = true,
            "-c" | "--commit" if syncing => commit = true,
            "-m" | "--message" if syncing => {
                let value = it
                    .next()
                    .ok_or_else(|| format!("`{arg}` needs a value"))?
                    .to_string();
                message = Some(value);
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option `{other}` for `submodule {sub}`"))
            }
            positional => {
                if dir.replace(PathBuf::from(positional)).is_some() {
                    return Err(format!("unexpected extra argument `{positional}`"));
                }
            }
        }
    }

    if message.is_some() && !commit {
        return Err("`--message` only applies with `--commit`".to_string());
    }

    let dir = dir.unwrap_or_else(|| PathBuf::from("."));
    Ok(if syncing {
        Cmd::Sync {
            dir,
            recursive,
            remote,
            commit,
            message,
            json,
        }
    } else {
        Cmd::Status {
            dir,
            recursive,
            json,
        }
    })
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn short(sha: &str) -> &str {
    if sha.len() > 7 {
        &sha[..7]
    } else {
        sha
    }
}

fn render_status_text(rows: &[SubmoduleStatusEntry]) -> String {
    if rows.is_empty() {
        return "no submodules\n".to_string();
    }
    let width = rows.iter().map(|r| r.state.len()).max().unwrap_or(0);
    let mut out = String::new();
    for row in rows {
        out.push_str(&format!(
            "{:<width$}  {}  {}",
            row.state,
            short(&row.commit),
            row.path,
            width = width
        ));
        if !row.detail.is_empty() {
            out.push_str(&format!("  {}", row.detail));
        }
        out.push('\n');
    }
    out
}

fn render_status_json(rows: &[SubmoduleStatusEntry]) -> String {
    let items: Vec<String> = rows
        .iter()
        .map(|r| {
            format!(
                r#"{{"path":"{}","state":"{}","commit":"{}","detail":"{}"}}"#,
                json_escape(&r.path),
                json_escape(&r.state),
                json_escape(&r.commit),
                json_escape(&r.detail)
            )
        })
        .collect();
    format!("[{}]\n", items.join(","))
}

fn render_sync_text(changed: &[SubmoduleChange], commit: Option<&str>) -> String {
    if changed.is_empty() {
        return "submodules already current\n".to_string();
    }
    let mut out = String::new();
    for change in changed {
        if change.initialized {
            out.push_str(&format!(
                "{}  initialized at {}",
                change.path,
                short(&change.to)
            ));
        } else {
            out.push_str(&format!(
                "{}  {} -> {}",
                change.path,
                short(&change.from),
                short(&change.to)
            ));
        }
        if !change.branch.is_empty() {
            out.push_str(&format!("  ({})", change.branch));
        }
        out.push('\n');
    }

    let n = changed.len();
    let plural = if n == 1 { "submodule" } else { "submodules" };
    match commit {
        Some(sha) => out.push_str(&format!(
            "\n{n} {plural} advanced, recorded as {}\n",
            short(sha)
        )),
        None => out.push_str(&format!(
            "\n{n} {plural} advanced (not recorded — re-run with --commit)\n"
        )),
    }
    out
}

fn render_sync_json(changed: &[SubmoduleChange], commit: Option<&str>) -> String {
    let items: Vec<String> = changed
        .iter()
        .map(|c| {
            format!(
                r#"{{"path":"{}","from":"{}","to":"{}","initialized":{},"branch":"{}"}}"#,
                json_escape(&c.path),
                json_escape(&c.from),
                json_escape(&c.to),
                c.initialized,
                json_escape(&c.branch)
            )
        })
        .collect();
    let commit_field = match commit {
        Some(sha) => format!(r#""{}""#, json_escape(sha)),
        None => "null".to_string(),
    };
    format!(
        r#"{{"changed":[{}],"commit":{}}}"#,
        items.join(","),
        commit_field
    ) + "\n"
}

fn run_status(dir: &Path, recursive: bool, json: bool) -> CliOutcome {
    let rows = submodule_status(dir, recursive);
    CliOutcome::ok(if json {
        render_status_json(&rows)
    } else {
        render_status_text(&rows)
    })
}

fn run_sync(
    dir: &Path,
    recursive: bool,
    remote: bool,
    commit: bool,
    message: Option<String>,
    json: bool,
) -> CliOutcome {
    let report = update_submodules(
        dir,
        &UpdateOptions {
            recursive,
            remote,
            commit,
            message,
        },
    );

    if !report.ok {
        return CliOutcome {
            code: 1,
            stdout: String::new(),
            stderr: format!("gitxtend: {}\n", report.stderr),
        };
    }

    CliOutcome::ok(if json {
        render_sync_json(&report.changed, report.commit.as_deref())
    } else {
        render_sync_text(&report.changed, report.commit.as_deref())
    })
}

/// Run the CLI over `args` (argv WITHOUT the program name).
pub fn run(args: &[String]) -> CliOutcome {
    match parse_args(args) {
        Ok(Cmd::Help) => CliOutcome::ok(USAGE.to_string()),
        Ok(Cmd::Version) => CliOutcome::ok(format!("gitxtend {}\n", env!("CARGO_PKG_VERSION"))),
        Ok(Cmd::Status {
            dir,
            recursive,
            json,
        }) => run_status(&dir, recursive, json),
        Ok(Cmd::Sync {
            dir,
            recursive,
            remote,
            commit,
            message,
            json,
        }) => run_sync(&dir, recursive, remote, commit, message, json),
        Err(msg) => CliOutcome {
            // 2 = bad usage, distinct from 1 = a git operation failed.
            code: 2,
            stdout: String::new(),
            stderr: format!("gitxtend: {msg}\n\n{USAGE}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(argv: &[&str]) -> Result<Cmd, String> {
        let owned: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
        parse_args(&owned)
    }

    fn sync(argv: &[&str]) -> Cmd {
        parse(argv).expect("parses")
    }

    #[test]
    fn no_args_prints_help() {
        assert_eq!(parse(&[]).unwrap(), Cmd::Help);
    }

    #[test]
    fn help_and_version_flags() {
        for a in ["-h", "--help", "help"] {
            assert_eq!(parse(&[a]).unwrap(), Cmd::Help, "{a}");
        }
        for a in ["-V", "--version", "version"] {
            assert_eq!(parse(&[a]).unwrap(), Cmd::Version, "{a}");
        }
        // --help anywhere in a subcommand still yields help rather than running.
        assert_eq!(sync(&["submodule", "--help"]), Cmd::Help);
        assert_eq!(sync(&["submodule", "sync", "--help"]), Cmd::Help);
    }

    #[test]
    fn sync_defaults_are_the_keep_up_to_date_configuration() {
        // The defaults ARE the feature: recurse, follow the tracked branch tip,
        // and do not write history without being asked.
        assert_eq!(
            sync(&["submodule", "sync"]),
            Cmd::Sync {
                dir: PathBuf::from("."),
                recursive: true,
                remote: true,
                commit: false,
                message: None,
                json: false,
            }
        );
    }

    #[test]
    fn update_is_an_alias_for_sync() {
        assert_eq!(sync(&["submodule", "update"]), sync(&["submodule", "sync"]));
        // and `submodules` for `submodule`
        assert_eq!(sync(&["submodules", "sync"]), sync(&["submodule", "sync"]));
    }

    #[test]
    fn sync_accepts_a_directory_and_flags() {
        assert_eq!(
            sync(&[
                "submodule",
                "sync",
                "/tmp/x",
                "--commit",
                "-m",
                "my msg",
                "--no-recursive",
                "--no-remote",
                "--json",
            ]),
            Cmd::Sync {
                dir: PathBuf::from("/tmp/x"),
                recursive: false,
                remote: false,
                commit: true,
                message: Some("my msg".to_string()),
                json: true,
            }
        );
    }

    #[test]
    fn flags_may_precede_the_directory() {
        let a = sync(&["submodule", "sync", "--commit", "/tmp/x"]);
        let b = sync(&["submodule", "sync", "/tmp/x", "--commit"]);
        assert_eq!(a, b);
    }

    #[test]
    fn status_parses_with_its_own_flags() {
        assert_eq!(
            sync(&["submodule", "status", "/tmp/x", "--no-recursive", "--json"]),
            Cmd::Status {
                dir: PathBuf::from("/tmp/x"),
                recursive: false,
                json: true,
            }
        );
    }

    #[test]
    fn usage_errors_are_reported_not_guessed_at() {
        // Unknown command / subcommand.
        assert!(parse(&["bogus"]).is_err());
        assert!(parse(&["submodule", "bogus"]).is_err());
        // `submodule` with no subcommand.
        assert!(parse(&["submodule"]).is_err());
        // Unknown option.
        assert!(parse(&["submodule", "sync", "--nope"]).is_err());
        // Sync-only options rejected on status, rather than silently ignored.
        assert!(parse(&["submodule", "status", "--commit"]).is_err());
        assert!(parse(&["submodule", "status", "--no-remote"]).is_err());
        // A value-taking flag at the end of argv.
        assert!(parse(&["submodule", "sync", "-m"]).is_err());
        // Two positionals.
        assert!(parse(&["submodule", "sync", "/a", "/b"]).is_err());
        // `--message` without `--commit` would silently do nothing.
        assert!(parse(&["submodule", "sync", "-m", "x"]).is_err());
    }

    #[test]
    fn json_escape_covers_the_characters_that_would_break_a_parse() {
        assert_eq!(json_escape(r#"a"b"#), r#"a\"b"#);
        assert_eq!(json_escape(r"a\b"), r"a\\b");
        assert_eq!(json_escape("a\nb"), "a\\nb");
        assert_eq!(json_escape("a\tb"), "a\\tb");
        assert_eq!(json_escape("a\u{1}b"), "a\\u0001b");
        assert_eq!(json_escape("plain/path-1"), "plain/path-1");
    }

    fn change(
        path: &str,
        from: &str,
        to: &str,
        initialized: bool,
        branch: &str,
    ) -> SubmoduleChange {
        SubmoduleChange {
            path: path.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            initialized,
            branch: branch.to_string(),
        }
    }

    #[test]
    fn sync_text_says_whether_the_bump_was_recorded() {
        let changed = vec![change("mod", "1111111aaa", "2222222bbb", false, "devel")];

        let uncommitted = render_sync_text(&changed, None);
        assert!(
            uncommitted.contains("mod  1111111 -> 2222222  (devel)"),
            "{uncommitted}"
        );
        assert!(
            uncommitted.contains("--commit"),
            "an unrecorded run must point at the flag that records it: {uncommitted}"
        );

        let committed = render_sync_text(&changed, Some("3333333ccc"));
        assert!(committed.contains("recorded as 3333333"), "{committed}");
        assert!(!committed.contains("--commit"), "{committed}");
    }

    #[test]
    fn sync_text_reports_an_initialized_submodule_distinctly() {
        // A first checkout can land on an unchanged SHA; "abc -> abc" would read
        // as a no-op when something did happen.
        let changed = vec![change("mod", "1111111aaa", "1111111aaa", true, "")];
        let out = render_sync_text(&changed, None);
        assert!(out.contains("initialized at 1111111"), "{out}");
        assert!(!out.contains("->"), "{out}");
    }

    #[test]
    fn sync_text_on_no_changes_says_so_and_never_mentions_commit() {
        let out = render_sync_text(&[], None);
        assert_eq!(out, "submodules already current\n");
    }

    #[test]
    fn sync_text_pluralizes() {
        let one = render_sync_text(&[change("a", "1", "2", false, "")], None);
        assert!(one.contains("1 submodule advanced"), "{one}");
        let two = render_sync_text(
            &[
                change("a", "1", "2", false, ""),
                change("b", "3", "4", false, ""),
            ],
            None,
        );
        assert!(two.contains("2 submodules advanced"), "{two}");
    }

    #[test]
    fn sync_json_shape() {
        let changed = vec![change("mod", "aaa", "bbb", false, "devel")];
        assert_eq!(
            render_sync_json(&changed, Some("ccc")),
            "{\"changed\":[{\"path\":\"mod\",\"from\":\"aaa\",\"to\":\"bbb\",\
             \"initialized\":false,\"branch\":\"devel\"}],\"commit\":\"ccc\"}\n"
        );
        assert_eq!(
            render_sync_json(&[], None),
            "{\"changed\":[],\"commit\":null}\n"
        );
    }

    fn row(path: &str, state: &str, commit: &str, detail: &str) -> SubmoduleStatusEntry {
        SubmoduleStatusEntry {
            path: path.to_string(),
            state: state.to_string(),
            commit: commit.to_string(),
            detail: detail.to_string(),
        }
    }

    #[test]
    fn status_text_aligns_and_handles_the_empty_case() {
        assert_eq!(render_status_text(&[]), "no submodules\n");

        let rows = vec![
            row("a", "clean", "1111111aaa", "(devel)"),
            row("b", "not-initialized", "2222222bbb", ""),
        ];
        let out = render_status_text(&rows);
        assert!(
            out.contains("clean            1111111  a  (devel)"),
            "{out}"
        );
        assert!(out.contains("not-initialized  2222222  b\n"), "{out}");
    }

    #[test]
    fn status_json_shape() {
        assert_eq!(render_status_json(&[]), "[]\n");
        assert_eq!(
            render_status_json(&[row("a", "clean", "1", "(devel)")]),
            "[{\"path\":\"a\",\"state\":\"clean\",\"commit\":\"1\",\"detail\":\"(devel)\"}]\n"
        );
    }
}
