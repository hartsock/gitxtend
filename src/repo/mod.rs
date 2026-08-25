//! Pure-Rust, gix-backed read primitives. NO PyO3 here — keep this module
//! testable with gix fixtures and reusable by an optional CLI bin target.
//!
//! ONE FILE PER METHOD. Each M1 task adds `src/repo/<name>.rs` (the gix
//! implementation + its parity tests) and registers it with a two-line block
//! here:
//!
//! ```ignore
//! mod is_git_repo;
//! pub use is_git_repo::is_git_repo;
//! ```
//!
//! so per-task PRs never collide on a shared function body. The matching PyO3
//! wrapper for the method is added separately in `src/python.rs`. Implement each
//! function per `docs/PORTING.md`, with parity tests vs the real `git` CLI.

#[allow(unused_imports)]
pub use crate::error::{GitxtendError, Result};

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

// ---- method registrations (one block per implemented method) -------------
// (methods land here as M1 progresses — see docs/ROADMAP.md M1 ordering)

mod is_git_repo;
pub use is_git_repo::is_git_repo;

mod head_sha;
pub use head_sha::head_sha;

mod current_branch;
pub use current_branch::current_branch;

mod tracking_branch;
pub use tracking_branch::tracking_branch;

mod remote_head_sha;
pub use remote_head_sha::remote_head_sha;

mod ahead_behind;
pub use ahead_behind::ahead_behind;

mod rev_list_count;
pub use rev_list_count::rev_list_count;

mod log_subjects;
pub use log_subjects::log_subjects;

mod is_clean;
pub use is_clean::is_clean;

mod status_counts;
pub use status_counts::status_counts;

mod remote_urls;
pub use remote_urls::remote_urls;

mod last_commit_date;
pub use last_commit_date::last_commit_date;

mod fetch;
pub use fetch::{fetch, fetch_result};

mod pull;
pub use pull::pull;

mod push;
pub use push::push;

mod add;
pub use add::add;

mod commit;
pub use commit::commit;

fn run_git<I, S>(path: &Path, args: I) -> (bool, String, String)
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(path).args(args).env("LC_ALL", "C");
    for key in AMBIENT_REPO_ENV {
        cmd.env_remove(key);
    }
    let out = cmd.output();

    match out {
        Ok(out) => (
            out.status.success(),
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
        ),
        Err(e) => (false, e.to_string(), String::new()),
    }
}

mod submodules;
pub use submodules::{
    submodule_status, sync_submodules, update_submodules, SubmoduleChange, SubmoduleStatusEntry,
    UpdateOptions, UpdateReport,
};

/// Environment variables by which an ambient git process points its children at
/// *its* repository, overriding `-C` / the current directory.
///
/// Every `git` invocation this crate spawns — fixtures included — must remove
/// them. A pre-push hook runs with `GIT_DIR` set, so a `git` child that inherits
/// it silently retargets the real repository: fixture `init`/`commit`/`checkout`
/// then land on the developer's own checkout rather than the temp dir.
pub(crate) const AMBIENT_REPO_ENV: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
];

/// Temp-dir git fixtures shared by the per-method parity tests.
///
/// Fixtures are built with the real `git` CLI, so each parity test asserts
/// "gix agrees with git on a repo git itself created"; the method under test
/// uses gix. See `docs/PORTING.md` → Testing strategy.
#[cfg(test)]
pub(crate) mod fixtures {
    use super::AMBIENT_REPO_ENV;
    use std::path::Path;
    use std::process::Command;
    use tempfile::TempDir;

    /// Build the `git` invocation a fixture uses, without running it.
    ///
    /// Split out from [`git`] so the env scrubbing below is directly assertable
    /// — see `fixture_git_scrubs_the_ambient_repo_env`. Setting `GIT_DIR` in a
    /// test to prove the behaviour is not an option: env vars are per-process,
    /// and these tests run in parallel threads.
    pub fn git_command(dir: &Path, args: &[&str]) -> Command {
        let mut cmd = Command::new("git");
        cmd.args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_AUTHOR_NAME", "fix")
            .env("GIT_AUTHOR_EMAIL", "fix@example.com")
            .env("GIT_COMMITTER_NAME", "fix")
            .env("GIT_COMMITTER_EMAIL", "fix@example.com");
        // Without this the fixture writes to whatever repo the ambient GIT_DIR
        // names — the developer's own, under a pre-push hook.
        for key in AMBIENT_REPO_ENV {
            cmd.env_remove(key);
        }
        cmd
    }

    /// Run a `git` subcommand in `dir`, assert success, return trimmed stdout.
    ///
    /// Global/system git config is neutralized and a fixed identity is set so
    /// fixtures are deterministic regardless of the host's `~/.gitconfig`.
    pub fn git(dir: &Path, args: &[&str]) -> String {
        let out = git_command(dir, args).output().expect("spawn git");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim_end().to_string()
    }

    /// A fresh repo on branch `main` with a single empty commit. Keep the
    /// returned `TempDir` alive for the duration of the test.
    pub fn repo() -> TempDir {
        let td = tempfile::tempdir().expect("tempdir");
        let p = td.path();
        git(p, &["init", "-q", "-b", "main"]);
        git(p, &["commit", "-q", "--allow-empty", "-m", "init"]);
        td
    }

    /// Write `contents` to `name` under `dir` (parent dirs created).
    pub fn write(dir: &Path, name: &str, contents: &str) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(path, contents).expect("write");
    }
}

#[cfg(test)]
mod tests {

    /// Regression: a fixture `git` must never inherit the ambient repo pointers.
    ///
    /// Discovered the hard way — `cargo test` under the pre-push hook (which
    /// runs with `GIT_DIR` set) retargeted every fixture command at the real
    /// checkout: it moved a branch onto a fixture commit, created a stray
    /// branch, and set `core.bare=true` + a `fix@example.com` identity in the
    /// developer's own config. `-C` and `current_dir` do NOT protect against
    /// this; `GIT_DIR` wins over both.
    ///
    /// Asserted on the built command rather than by setting `GIT_DIR` for real,
    /// because env vars are per-process and these tests run in parallel.
    #[test]
    fn fixture_git_scrubs_the_ambient_repo_env() {
        let td = tempfile::tempdir().expect("tempdir");
        let cmd = fixtures::git_command(td.path(), &["status"]);

        let removed: Vec<&str> = cmd
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .filter_map(|(key, _)| key.to_str())
            .collect();

        for key in super::AMBIENT_REPO_ENV {
            assert!(
                removed.contains(key),
                "fixture git must remove {key}; removes {removed:?}"
            );
        }
    }
    use super::fixtures;

    #[test]
    fn fixture_repo_has_one_commit() {
        let td = fixtures::repo();
        assert_eq!(
            fixtures::git(td.path(), &["rev-list", "--count", "HEAD"]),
            "1"
        );
    }

    #[test]
    fn fixture_write_creates_file() {
        let td = fixtures::repo();
        fixtures::write(td.path(), "a/b.txt", "hi");
        assert_eq!(
            std::fs::read_to_string(td.path().join("a/b.txt")).unwrap(),
            "hi"
        );
    }
}
