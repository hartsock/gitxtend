use crate::error::GitxtendError;
use crate::repo::Result;
use std::path::Path;

/// HEAD commit's AUTHOR date as strict ISO-8601 (matches `git log -1
/// --format=%aI`, e.g. "2026-05-31T13:45:30-04:00"); `Ok(None)` on an
/// unborn/empty repo.
pub fn last_commit_date(path: &Path) -> Result<Option<String>> {
    let repo = gix::open(path).map_err(GitxtendError::from_err)?;
    let commit = match repo.head_commit() {
        Ok(c) => c,
        Err(_) => return Ok(None), // unborn HEAD
    };
    let sig = commit.author().map_err(GitxtendError::from_err)?;
    Ok(Some(
        sig.time.format(gix::date::time::format::ISO8601_STRICT),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures::{git, repo, write};

    /// git's `%aI` renders a zero (UTC) offset as `Z` on newer git but `+00:00`
    /// on older git — a host-git-version quirk. gitxtend is deterministic and
    /// always emits `+00:00`, so normalize git's output to compare the same
    /// instant regardless of the host git version (see DESIGN.md: no dependence
    /// on the ambient git version).
    fn norm(s: String) -> String {
        match s.strip_suffix('Z') {
            Some(stripped) => format!("{stripped}+00:00"),
            None => s,
        }
    }

    #[test]
    fn test_last_commit_date_initial_repo() {
        let td = repo();
        let path = td.path();
        assert_eq!(
            last_commit_date(path).unwrap(),
            Some(norm(git(path, &["log", "-1", "--format=%aI"])))
        );
    }

    #[test]
    fn test_last_commit_date_after_second_commit() {
        let td = repo();
        let path = td.path();
        write(path, "file.txt", "content");
        git(path, &["add", "file.txt"]);
        git(path, &["commit", "-q", "-m", "second commit"]);
        assert_eq!(
            last_commit_date(path).unwrap(),
            Some(norm(git(path, &["log", "-1", "--format=%aI"])))
        );
    }

    #[test]
    fn test_last_commit_date_unborn_repo() {
        let td = tempfile::tempdir().expect("tempdir");
        let path = td.path();
        git(path, &["init", "-q", "-b", "main"]);
        assert_eq!(last_commit_date(path).unwrap(), None);
    }
}
