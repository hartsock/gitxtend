use crate::error::GitxtendError;
use crate::repo::Result;
use std::path::Path;

/// True iff the working tree is clean — no staged, modified, or untracked
/// entries — i.e. `git status --porcelain` would be empty.
pub fn is_clean(path: &Path) -> Result<bool> {
    let repo = gix::open(path).map_err(GitxtendError::from_err)?;
    let mut iter = repo
        .status(gix::progress::Discard)
        .map_err(GitxtendError::from_err)?
        .untracked_files(gix::status::UntrackedFiles::Files)
        .into_iter(Vec::<gix::bstr::BString>::new())
        .map_err(GitxtendError::from_err)?;
    // clean == the status iterator yields no entries (staged, modified, or untracked)
    Ok(iter.next().is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures::{self, repo, write};

    fn porcelain_clean(p: &std::path::Path) -> bool {
        fixtures::git(p, &["status", "--porcelain"]).is_empty()
    }

    #[test]
    fn test_is_clean_fresh_repo() {
        let td = repo();
        assert!(is_clean(td.path()).unwrap());
        assert_eq!(is_clean(td.path()).unwrap(), porcelain_clean(td.path()));
    }

    #[test]
    fn test_is_clean_untracked_file() {
        let td = repo();
        write(td.path(), "new.txt", "x");
        assert!(!is_clean(td.path()).unwrap());
        assert_eq!(is_clean(td.path()).unwrap(), porcelain_clean(td.path()));
    }

    #[test]
    fn test_is_clean_modified_tracked_file() {
        let td = repo();
        write(td.path(), "README.md", "orig");
        fixtures::git(td.path(), &["add", "README.md"]);
        fixtures::git(td.path(), &["commit", "-m", "Add README"]);
        write(td.path(), "README.md", "Modified content");
        assert!(!is_clean(td.path()).unwrap());
        assert_eq!(is_clean(td.path()).unwrap(), porcelain_clean(td.path()));
    }

    #[test]
    fn test_is_clean_staged_change() {
        let td = repo();
        write(td.path(), "new.txt", "x");
        fixtures::git(td.path(), &["add", "new.txt"]);
        assert!(!is_clean(td.path()).unwrap());
        assert_eq!(is_clean(td.path()).unwrap(), porcelain_clean(td.path()));
    }
}
