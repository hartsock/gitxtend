//! Submodule helpers.
//!
//! Unlike most v1 methods (which are pure gix), submodule update/status is
//! intentionally delegated to the local `git` CLI so we follow Git's own
//! submodule semantics exactly.

use std::path::Path;
use std::process::Command;

/// One row from `git submodule status`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmoduleStatusEntry {
    /// Submodule path inside the superproject.
    pub path: String,
    /// One of `clean`, `not-initialized`, `out-of-date`, `unmerged`, `unknown`.
    pub state: String,
    /// Reported commit SHA (or placeholder hash from the superproject index).
    pub commit: String,
    /// Optional extra detail (usually `(branch)`).
    pub detail: String,
}

fn status_state(marker: char) -> &'static str {
    match marker {
        ' ' => "clean",
        '-' => "not-initialized",
        '+' => "out-of-date",
        'U' => "unmerged",
        _ => "unknown",
    }
}

fn parse_status_line(line: &str) -> Option<SubmoduleStatusEntry> {
    if line.is_empty() {
        return None;
    }

    let mut chars = line.chars();
    let marker = chars.next()?;
    let remainder = chars.as_str().trim_start();

    let mut pieces = remainder.splitn(3, ' ');
    let commit = pieces.next()?.trim();
    let path = pieces.next()?.trim();
    let detail = pieces.next().unwrap_or("").trim();

    if commit.is_empty() || path.is_empty() {
        return None;
    }

    Some(SubmoduleStatusEntry {
        path: path.to_string(),
        state: status_state(marker).to_string(),
        commit: commit.to_string(),
        detail: detail.to_string(),
    })
}

/// Parse `git submodule status` output into structured rows.
/// Returns `[]` on any command/path error (soft-fail behavior, matching v1 style).
pub fn submodule_status(path: &Path, recursive: bool) -> Vec<SubmoduleStatusEntry> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(path)
        .args(["submodule", "status"])
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    if recursive {
        cmd.arg("--recursive");
    }

    let output = match cmd.output() {
        Ok(out) if out.status.success() => out,
        Ok(_) | Err(_) => return Vec::new(),
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_status_line)
        .collect()
}

/// Run `git submodule update --init` for the repo.
/// Returns `(ok, stderr)` so caller can surface errors.
pub fn sync_submodules(path: &Path, recursive: bool, update_remote: bool) -> (bool, String) {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(path)
        .args(["submodule", "update", "--init"])
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    if recursive {
        cmd.arg("--recursive");
    }
    if update_remote {
        cmd.arg("--remote");
    }

    match cmd.output() {
        Ok(out) => (
            out.status.success(),
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ),
        Err(e) => (false, e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures;
    use std::fs;
    use std::process::Command;

    fn git_allow_file_protocol(path: &Path, args: &[&str]) -> String {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(path)
            .arg("-c")
            .arg("protocol.file.allow=always")
            .args(args);
        let out = cmd.output().expect("git submodule command");
        if !out.status.success() {
            panic!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        String::from_utf8_lossy(&out.stdout).trim_end().to_string()
    }

    fn git_submodule_add(parent: &Path, child: &Path, path: &str) {
        git_allow_file_protocol(parent, &["submodule", "add", child.to_str().unwrap(), path]);
    }

    #[test]
    fn parse_status_state_markers() {
        let cases = vec![
            (" 1234567890abcdef1234567890abcdef12345678 path", "clean"),
            (
                "-1234567890abcdef1234567890abcdef12345678 path",
                "not-initialized",
            ),
            (
                "+1234567890abcdef1234567890abcdef12345678 path (main)",
                "out-of-date",
            ),
            ("U1234567890abcdef1234567890abcdef12345678 path", "unmerged"),
            ("?1234567890abcdef1234567890abcdef12345678 path", "unknown"),
        ];

        for (line, expected) in cases {
            let entry = parse_status_line(line).expect("parseable");
            assert_eq!(entry.state, expected);
            assert_eq!(entry.path, "path");
            assert_eq!(entry.commit, "1234567890abcdef1234567890abcdef12345678");
        }
    }

    fn git_submodule_status(repo: &Path, recursive: bool) -> String {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(repo).arg("submodule").arg("status");
        if recursive {
            cmd.arg("--recursive");
        }
        let out = cmd.output().expect("git status command");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim_end().to_string()
    }

    #[test]
    fn submodule_status_reports_not_initialized_and_clean() {
        let parent = fixtures::repo();

        let child = tempfile::tempdir().unwrap();
        fixtures::git(child.path(), &["init", "-q", "-b", "main"]);
        fixtures::write(child.path(), "hello.txt", "hi");
        fixtures::git(child.path(), &["add", "-A"]);
        fixtures::git(child.path(), &["commit", "-q", "-m", "child init"]);

        git_submodule_add(parent.path(), child.path(), "child-module");
        fixtures::git(parent.path(), &["add", "-A"]);
        fixtures::git(
            parent.path(),
            &["commit", "-q", "-m", "add child submodule"],
        );

        let rows_clean = submodule_status(parent.path(), false);
        assert_eq!(rows_clean.len(), 1);
        assert_eq!(rows_clean[0].state, "clean");

        let status = git_submodule_status(parent.path(), false);
        assert!(!status.is_empty());

        fs::remove_dir_all(parent.path().join("child-module")).unwrap();
        let rows_missing = submodule_status(parent.path(), false);
        assert_eq!(rows_missing.len(), 1);
        assert_eq!(rows_missing[0].state, "not-initialized");
    }

    #[test]
    fn sync_submodules_reinitializes_submodule() {
        let parent = fixtures::repo();

        let child = tempfile::tempdir().unwrap();
        fixtures::git(child.path(), &["init", "-q", "-b", "main"]);
        fixtures::write(child.path(), "hello.txt", "hi");
        fixtures::git(child.path(), &["add", "-A"]);
        fixtures::git(child.path(), &["commit", "-q", "-m", "child init"]);

        git_submodule_add(parent.path(), child.path(), "child-module");
        fixtures::git(parent.path(), &["add", "-A"]);
        fixtures::git(
            parent.path(),
            &["commit", "-q", "-m", "add child submodule"],
        );

        fs::remove_dir_all(parent.path().join("child-module")).unwrap();
        let (ok, stderr) = sync_submodules(parent.path(), false, false);
        assert!(ok, "{stderr}");
        assert!(parent.path().join("child-module").exists());
        assert_eq!(submodule_status(parent.path(), false)[0].state, "clean");
    }

    #[test]
    fn sync_submodules_on_repo_with_no_submodules_still_ok() {
        let td = fixtures::repo();
        assert!(sync_submodules(td.path(), false, false).0);
    }
}
