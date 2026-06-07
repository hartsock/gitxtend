use std::path::Path;

/// Run `git commit -m <message>` in `path`.
///
/// A clean worktree with "nothing to commit" is treated as success to match
/// git-tend's write-side contract.
pub fn commit(path: &Path, message: &str) -> (bool, String) {
    let (ok, stderr, stdout) = super::run_git(path, ["commit", "-m", message]);
    if ok {
        return (true, stderr);
    }

    let output = format!("{stdout}\n{stderr}").to_ascii_lowercase();
    if output.contains("nothing to commit") {
        (true, String::new())
    } else {
        (false, stderr)
    }
}

#[cfg(test)]
mod tests {
    use crate::repo::fixtures;

    // These tests exercise `run_git`, which intentionally preserves host git
    // config for production parity. Set repo-local config when behavior depends
    // on identity or other git settings.

    #[test]
    fn commit_creates_commit_with_message() {
        let repo = fixtures::repo();
        fixtures::git(repo.path(), &["config", "user.name", "qa"]);
        fixtures::git(repo.path(), &["config", "user.email", "qa@example.com"]);
        fixtures::write(repo.path(), "new.txt", "new\n");
        fixtures::git(repo.path(), &["add", "-A"]);

        let (ok, stderr) = super::commit(repo.path(), "new commit");

        assert!(ok, "{stderr}");
        assert!(stderr.is_empty(), "{stderr}");
        assert_eq!(
            fixtures::git(repo.path(), &["log", "-1", "--format=%s"]),
            "new commit"
        );
    }

    #[test]
    fn commit_nothing_to_commit_counts_as_success() {
        let repo = fixtures::repo();
        fixtures::git(repo.path(), &["config", "user.name", "qa"]);
        fixtures::git(repo.path(), &["config", "user.email", "qa@example.com"]);
        let before = fixtures::git(repo.path(), &["rev-parse", "HEAD"]);

        let (ok, stderr) = super::commit(repo.path(), "noop");

        assert!(ok, "{stderr}");
        assert!(stderr.is_empty(), "{stderr}");
        assert_eq!(
            fixtures::git(repo.path(), &["rev-parse", "HEAD"]),
            before,
            "noop commit must not create a new commit"
        );
    }
}
