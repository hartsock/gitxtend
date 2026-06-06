use std::path::Path;

/// Run `git push <remote>` in `path`.
pub fn push(path: &Path, remote: &str) -> (bool, String) {
    let (ok, stderr, _) = super::run_git(path, ["push", remote]);
    (ok, stderr)
}

#[cfg(test)]
mod tests {
    use crate::repo::fixtures;

    #[test]
    fn push_sends_local_commit_to_origin() {
        let repo = fixtures::repo();
        let bare = tempfile::tempdir().unwrap();
        fixtures::git(bare.path(), &["init", "--bare", "-q", "-b", "main"]);
        let bare_url = bare.path().to_string_lossy().to_string();
        fixtures::git(repo.path(), &["remote", "add", "origin", &bare_url]);
        fixtures::git(repo.path(), &["push", "-q", "-u", "origin", "main"]);
        fixtures::git(repo.path(), &["config", "push.default", "upstream"]);

        fixtures::write(repo.path(), "local.txt", "local\n");
        fixtures::git(repo.path(), &["add", "-A"]);
        fixtures::git(repo.path(), &["commit", "-q", "-m", "local"]);

        let (ok, stderr) = super::push(repo.path(), "origin");

        assert!(ok, "{stderr}");
        assert_eq!(
            fixtures::git(repo.path(), &["rev-parse", "HEAD"]),
            fixtures::git(bare.path(), &["rev-parse", "main"])
        );
    }

    #[test]
    fn push_bad_remote_returns_failure_and_stderr() {
        let repo = fixtures::repo();

        let (ok, stderr) = super::push(repo.path(), "does-not-exist");

        assert!(!ok);
        assert!(!stderr.is_empty());
    }
}
