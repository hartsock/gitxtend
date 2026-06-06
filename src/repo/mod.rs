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
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env("LC_ALL", "C")
        .output();

    match out {
        Ok(out) => (
            out.status.success(),
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
        ),
        Err(e) => (false, e.to_string(), String::new()),
    }
}

/// Temp-dir git fixtures shared by the per-method parity tests.
///
/// Fixtures are built with the real `git` CLI, so each parity test asserts
/// "gix agrees with git on a repo git itself created"; the method under test
/// uses gix. See `docs/PORTING.md` → Testing strategy.
#[cfg(test)]
pub(crate) mod fixtures {
    use std::path::Path;
    use std::process::Command;
    use tempfile::TempDir;

    /// Run a `git` subcommand in `dir`, assert success, return trimmed stdout.
    ///
    /// Global/system git config is neutralized and a fixed identity is set so
    /// fixtures are deterministic regardless of the host's `~/.gitconfig`.
    pub fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_AUTHOR_NAME", "fix")
            .env("GIT_AUTHOR_EMAIL", "fix@example.com")
            .env("GIT_COMMITTER_NAME", "fix")
            .env("GIT_COMMITTER_EMAIL", "fix@example.com")
            .output()
            .expect("spawn git");
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
