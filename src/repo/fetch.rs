//! `fetch` — the one network call in the v1 read-side scope.
//!
//! Implemented as a contained `git fetch` shell-out rather than via gix. Per
//! docs/PORTING.md, gix's network fetch is the least-mature path in scope; the
//! shared `run_git` shell-out runs the user's own `git`, so it honors their
//! config, credentials, and ssh-agent exactly. `fetch_result` exposes
//! `(ok, stderr)` so the `repo_status` roll-up can report *why* a fetch failed
//! (docs/API.md).

use std::path::Path;

/// Run `git fetch` in `path` and return `(ok, stderr)`. `remote = None` fetches
/// all remotes (`git fetch --all`); `Some(name)` fetches that one remote.
pub fn fetch_result(path: &Path, remote: Option<&str>) -> (bool, String) {
    let args = match remote {
        Some(r) => vec!["fetch", r],
        None => vec!["fetch", "--all"],
    };
    let (ok, stderr, _) = super::run_git(path, args);
    (ok, stderr)
}

/// Fetch from `remote` (or all remotes when `None`). Returns true on success.
pub fn fetch(path: &Path, remote: Option<&str>) -> bool {
    fetch_result(path, remote).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures;

    #[test]
    fn fetch_updates_tracking_ref() {
        // repo A with a local bare remote
        let a = fixtures::repo();
        let bare = tempfile::tempdir().unwrap();
        fixtures::git(bare.path(), &["init", "--bare", "-q", "-b", "main"]);
        let bare_url = bare.path().to_string_lossy().to_string();
        fixtures::git(a.path(), &["remote", "add", "origin", &bare_url]);
        fixtures::git(a.path(), &["push", "-q", "-u", "origin", "main"]);

        // advance the bare remote via a second clone
        let c = tempfile::tempdir().unwrap();
        fixtures::git(c.path(), &["clone", "-q", &bare_url, "."]);
        fixtures::write(c.path(), "new.txt", "x");
        fixtures::git(c.path(), &["add", "-A"]);
        fixtures::git(c.path(), &["commit", "-q", "-m", "remote commit"]);
        fixtures::git(c.path(), &["push", "-q", "origin", "main"]);

        // back in A: fetch should succeed and advance origin/main to C's HEAD
        assert!(fetch(a.path(), None));
        assert_eq!(
            fixtures::git(a.path(), &["rev-parse", "origin/main"]),
            fixtures::git(c.path(), &["rev-parse", "HEAD"])
        );
    }

    #[test]
    fn fetch_bad_remote_fails() {
        let a = fixtures::repo();
        let (ok, stderr) = fetch_result(a.path(), Some("does-not-exist"));
        assert!(!ok);
        assert!(!stderr.is_empty());
    }
}
