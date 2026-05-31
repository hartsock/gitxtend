//! `repo_status` roll-up — ports the source `check_repo` verbatim (see
//! docs/PORTING.md / docs/API.md). NO PyO3 here; `python.rs` converts the plain
//! [`RepoStatusData`] into the `#[pyclass] RepoStatus`.
//!
//! The struct field is `sync_state` (not `state`). The decision gates on **SHA
//! equality first** and **never returns `"dirty"`** — `is_dirty` is recorded as
//! a flag only; the `"dirty"` state belongs to the separate *scan* path (a later
//! milestone), per #11.

use crate::repo;
use std::path::Path;

/// Plain-Rust roll-up mirroring the source `RepoStatus` / `check_repo`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoStatusData {
    pub path: String,
    pub sync_state: String,
    pub local_branch: Option<String>,
    pub tracking_branch: Option<String>,
    pub local_sha: Option<String>,
    pub remote_sha: Option<String>,
    pub ahead_count: usize,
    pub behind_count: usize,
    pub new_remote_commits: Vec<String>,
    pub is_dirty: bool,
    pub error: Option<String>,
}

/// Mirror of `StatusService.check_repo`. When `fetch` is true, fetches first; a
/// fetch failure becomes `sync_state = "error"` with `error = "Fetch failed: …"`.
/// Error strings are identical to the source — tests assert on them.
pub fn repo_status(path: &Path, fetch: bool) -> RepoStatusData {
    let path_str = path.display().to_string();
    let mut s = RepoStatusData {
        path: path_str.clone(),
        ..Default::default()
    };

    if !path.exists() {
        s.sync_state = "error".to_string();
        s.error = Some(format!("Directory not found: {path_str}"));
        return s;
    }
    if !repo::is_git_repo(path) {
        s.sync_state = "error".to_string();
        s.error = Some(format!("Not a git repository: {path_str}"));
        return s;
    }

    s.local_branch = repo::current_branch(path).unwrap_or(None);

    let tracking = match repo::tracking_branch(path).unwrap_or(None) {
        Some(t) => t,
        None => {
            // No upstream: fill local fields; `tracking_branch` stays None.
            s.sync_state = "no-remote".to_string();
            s.local_sha = repo::head_sha(path).unwrap_or(None);
            s.is_dirty = !repo::is_clean(path).unwrap_or(true);
            return s;
        }
    };
    s.tracking_branch = Some(tracking.clone());

    if fetch {
        let (ok, stderr) = repo::fetch_result(path, None);
        if !ok {
            s.sync_state = "error".to_string();
            s.error = Some(format!("Fetch failed: {stderr}"));
            return s;
        }
    }

    s.local_sha = repo::head_sha(path).unwrap_or(None);
    s.remote_sha = repo::remote_head_sha(path, &tracking).unwrap_or(None);
    s.is_dirty = !repo::is_clean(path).unwrap_or(true);

    // ahead_count / behind_count are always populated.
    let ahead = repo::rev_list_count(path, &format!("{tracking}..HEAD"));
    let behind = repo::rev_list_count(path, &format!("HEAD..{tracking}"));
    s.ahead_count = ahead;
    s.behind_count = behind;

    s.sync_state = if s.local_sha == s.remote_sha {
        "up-to-date".to_string()
    } else if ahead > 0 && behind > 0 {
        "diverged".to_string()
    } else if behind > 0 {
        "behind".to_string()
    } else {
        "ahead".to_string()
    };

    if behind > 0 {
        s.new_remote_commits = repo::log_subjects(path, &format!("HEAD..{tracking}"), 10);
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures;

    /// A repo with a pushed `origin/main` (in sync). Returns (repo, bare-remote).
    fn repo_with_remote() -> (tempfile::TempDir, tempfile::TempDir) {
        let a = fixtures::repo();
        let bare = tempfile::tempdir().unwrap();
        fixtures::git(bare.path(), &["init", "--bare", "-q", "-b", "main"]);
        let url = bare.path().to_string_lossy().to_string();
        fixtures::git(a.path(), &["remote", "add", "origin", &url]);
        fixtures::git(a.path(), &["push", "-q", "-u", "origin", "main"]);
        (a, bare)
    }

    /// Clone `bare`, add a commit, push it — advances the remote `main`.
    fn advance_remote(bare: &Path) {
        let c = tempfile::tempdir().unwrap();
        let url = bare.to_string_lossy().to_string();
        fixtures::git(c.path(), &["clone", "-q", &url, "."]);
        fixtures::write(c.path(), "r.txt", "r");
        fixtures::git(c.path(), &["add", "-A"]);
        fixtures::git(c.path(), &["commit", "-q", "-m", "remote"]);
        fixtures::git(c.path(), &["push", "-q", "origin", "main"]);
    }

    #[test]
    fn error_directory_not_found() {
        let s = repo_status(Path::new("/definitely/not/a/real/path/xyzzy"), false);
        assert_eq!(s.sync_state, "error");
        assert!(s.error.unwrap().starts_with("Directory not found:"));
    }

    #[test]
    fn error_not_a_git_repository() {
        let td = tempfile::tempdir().unwrap();
        let s = repo_status(td.path(), false);
        assert_eq!(s.sync_state, "error");
        assert!(s.error.unwrap().starts_with("Not a git repository:"));
    }

    #[test]
    fn no_remote() {
        let td = fixtures::repo();
        let s = repo_status(td.path(), false);
        assert_eq!(s.sync_state, "no-remote");
        assert!(s.tracking_branch.is_none()); // left None per the spec
        assert!(s.local_sha.is_some());
        assert_eq!(s.local_branch.as_deref(), Some("main"));
    }

    #[test]
    fn up_to_date() {
        let (a, _bare) = repo_with_remote();
        let s = repo_status(a.path(), false);
        assert_eq!(s.sync_state, "up-to-date");
        assert_eq!(s.tracking_branch.as_deref(), Some("origin/main"));
        assert_eq!(s.local_sha, s.remote_sha);
        assert_eq!((s.ahead_count, s.behind_count), (0, 0));
    }

    #[test]
    fn ahead() {
        let (a, _bare) = repo_with_remote();
        fixtures::write(a.path(), "x.txt", "x");
        fixtures::git(a.path(), &["add", "-A"]);
        fixtures::git(a.path(), &["commit", "-q", "-m", "local"]);
        let s = repo_status(a.path(), false);
        assert_eq!(s.sync_state, "ahead");
        assert_eq!((s.ahead_count, s.behind_count), (1, 0));
    }

    #[test]
    fn behind() {
        let (a, bare) = repo_with_remote();
        advance_remote(bare.path());
        let s = repo_status(a.path(), true); // fetch=true picks up the new commit
        assert_eq!(s.sync_state, "behind");
        assert_eq!((s.ahead_count, s.behind_count), (0, 1));
        assert_eq!(s.new_remote_commits, vec!["remote".to_string()]);
    }

    #[test]
    fn diverged() {
        let (a, bare) = repo_with_remote();
        // local commit in A
        fixtures::write(a.path(), "l.txt", "l");
        fixtures::git(a.path(), &["add", "-A"]);
        fixtures::git(a.path(), &["commit", "-q", "-m", "local"]);
        // remote advances independently
        advance_remote(bare.path());
        let s = repo_status(a.path(), true);
        assert_eq!(s.sync_state, "diverged");
        assert_eq!((s.ahead_count, s.behind_count), (1, 1));
    }
}
