use std::path::Path;

/// Run `git add <pattern>` in `path`.
pub fn add(path: &Path, pattern: &str) -> (bool, String) {
    let (ok, stderr, _) = super::run_git(path, ["add", pattern]);
    (ok, stderr)
}

#[cfg(test)]
mod tests {
    use crate::repo::fixtures;

    #[test]
    fn add_stages_matching_path() {
        let repo = fixtures::repo();
        fixtures::write(repo.path(), "tracked.txt", "tracked\n");

        let (ok, stderr) = super::add(repo.path(), "tracked.txt");

        assert!(ok, "{stderr}");
        assert_eq!(
            fixtures::git(repo.path(), &["diff", "--cached", "--name-only"]),
            "tracked.txt"
        );
    }

    #[test]
    fn add_missing_path_returns_failure_and_stderr() {
        let repo = fixtures::repo();

        let (ok, stderr) = super::add(repo.path(), "missing.txt");

        assert!(!ok);
        assert!(!stderr.is_empty());
    }
}
