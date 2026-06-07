use std::path::Path;

/// Run `git pull` in `path`; include `--ff-only` when requested.
pub fn pull(path: &Path, ff_only: bool) -> (bool, String) {
    let args = if ff_only {
        vec!["pull", "--ff-only"]
    } else {
        vec!["pull"]
    };
    let (ok, stderr, _) = super::run_git(path, args);
    (ok, stderr)
}

#[cfg(test)]
mod tests {
    use crate::repo::fixtures;

    #[test]
    fn pull_ff_only_fast_forwards_from_origin() {
        let repo = fixtures::repo();
        let bare = tempfile::tempdir().unwrap();
        fixtures::git(bare.path(), &["init", "--bare", "-q", "-b", "main"]);
        let bare_url = bare.path().to_string_lossy().to_string();
        fixtures::git(repo.path(), &["remote", "add", "origin", &bare_url]);
        fixtures::git(repo.path(), &["push", "-q", "-u", "origin", "main"]);

        let clone = tempfile::tempdir().unwrap();
        fixtures::git(clone.path(), &["clone", "-q", &bare_url, "."]);
        fixtures::write(clone.path(), "remote.txt", "remote\n");
        fixtures::git(clone.path(), &["add", "-A"]);
        fixtures::git(clone.path(), &["commit", "-q", "-m", "remote"]);
        fixtures::git(clone.path(), &["push", "-q", "origin", "main"]);

        let (ok, stderr) = super::pull(repo.path(), true);

        assert!(ok, "{stderr}");
        assert_eq!(
            fixtures::git(repo.path(), &["rev-parse", "HEAD"]),
            fixtures::git(clone.path(), &["rev-parse", "HEAD"])
        );
    }

    #[test]
    fn pull_bad_repository_returns_failure_and_stderr() {
        let dir = tempfile::tempdir().unwrap();

        let (ok, stderr) = super::pull(dir.path(), true);

        assert!(!ok);
        assert!(!stderr.is_empty());
    }
}
