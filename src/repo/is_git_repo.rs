use gix::discover;
use std::path::Path;

/// Returns true iff `path` is inside a git working tree (mirrors
/// `git rev-parse --git-dir` exit==0).
pub fn is_git_repo(path: &Path) -> bool {
    discover(path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures;

    #[test]
    fn repo_root_and_subdir() {
        let td = fixtures::repo();
        let root_path = td.path();

        // Test at the repo root
        assert!(is_git_repo(root_path));

        // Create a subdirectory and test there
        let subdir_path = root_path.join("subdir");
        std::fs::create_dir(&subdir_path).expect("mkdir");
        assert!(is_git_repo(&subdir_path));
    }

    #[test]
    fn non_repo() {
        let td = tempfile::tempdir().expect("tempdir");
        let non_repo_path = td.path();
        assert!(!is_git_repo(non_repo_path));
    }

    /// Ask the `git` CLI whether `dir` is inside a repository.
    ///
    /// Built through `fixtures::git_command` so the oracle is scrubbed of the
    /// ambient `GIT_DIR`. A raw `Command::new("git")` here answered about
    /// whatever repo the environment pointed at — under a pre-push hook, the
    /// developer's own — so the parity assertion compared gix's answer about
    /// the temp dir against git's answer about a different repository.
    fn git_says_repo(dir: &std::path::Path) -> bool {
        fixtures::git_command(dir, &["rev-parse", "--git-dir"])
            .status()
            .expect("spawn git")
            .success()
    }

    #[test]
    fn parity_with_git_cli() {
        let repo_td = fixtures::repo();
        let repo_path = repo_td.path();

        // Check the repo path
        assert_eq!(is_git_repo(repo_path), git_says_repo(repo_path));

        // Check a subdirectory within the repo
        let subdir_path = repo_path.join("subdir");
        std::fs::create_dir(&subdir_path).expect("mkdir");
        assert_eq!(is_git_repo(&subdir_path), git_says_repo(&subdir_path));

        // Check a non-repo path
        let non_repo_td = tempfile::tempdir().expect("tempdir");
        let non_repo_path = non_repo_td.path();
        assert_eq!(is_git_repo(non_repo_path), git_says_repo(non_repo_path));
    }
}
