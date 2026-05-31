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
    use std::process::Command;

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

    #[test]
    fn parity_with_git_cli() {
        let repo_td = fixtures::repo();
        let repo_path = repo_td.path();

        // Check the repo path
        assert_eq!(
            is_git_repo(repo_path),
            Command::new("git")
                .args(["-C", &repo_path.to_string_lossy(), "rev-parse", "--git-dir"])
                .status()
                .expect("spawn git")
                .success()
        );

        // Check a subdirectory within the repo
        let subdir_path = repo_path.join("subdir");
        std::fs::create_dir(&subdir_path).expect("mkdir");
        assert_eq!(
            is_git_repo(&subdir_path),
            Command::new("git")
                .args([
                    "-C",
                    &subdir_path.to_string_lossy(),
                    "rev-parse",
                    "--git-dir"
                ])
                .status()
                .expect("spawn git")
                .success()
        );

        // Check a non-repo path
        let non_repo_td = tempfile::tempdir().expect("tempdir");
        let non_repo_path = non_repo_td.path();
        assert_eq!(
            is_git_repo(non_repo_path),
            Command::new("git")
                .args([
                    "-C",
                    &non_repo_path.to_string_lossy(),
                    "rev-parse",
                    "--git-dir"
                ])
                .status()
                .expect("spawn git")
                .success()
        );
    }
}
