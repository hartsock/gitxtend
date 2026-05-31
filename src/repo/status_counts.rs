use std::path::Path;

/// (modified, untracked) counts, matching `git status --porcelain`: lines
/// starting with `??` are untracked; every other non-empty entry (staged or
/// unstaged change, rename, delete) is modified. Soft-fails to (0, 0).
pub fn status_counts(path: &Path) -> (usize, usize) {
    let Ok(repo) = gix::open(path) else {
        return (0, 0);
    };
    let platform = match repo.status(gix::progress::Discard) {
        Ok(p) => p,
        Err(_) => return (0, 0),
    };
    let iter = match platform
        .untracked_files(gix::status::UntrackedFiles::Files)
        .into_iter(Vec::<gix::bstr::BString>::new())
    {
        Ok(i) => i,
        Err(_) => return (0, 0),
    };
    let mut modified = 0usize;
    let mut untracked = 0usize;
    for item in iter {
        let Ok(item) = item else { continue };
        match item {
            gix::status::Item::IndexWorktree(
                gix::status::index_worktree::Item::DirectoryContents { .. },
            ) => untracked += 1,
            _ => modified += 1,
        }
    }
    (modified, untracked)
}

#[cfg(test)]
mod tests {
    use crate::repo::fixtures;

    fn porcelain_counts(p: &std::path::Path) -> (usize, usize) {
        let out = fixtures::git(p, &["status", "--porcelain"]);
        let mut m = 0usize;
        let mut u = 0usize;
        for line in out.lines() {
            if line.is_empty() {
                continue;
            }
            if line.starts_with("??") {
                u += 1;
            } else {
                m += 1;
            }
        }
        (m, u)
    }

    #[test]
    fn clean_repo() {
        let td = fixtures::repo();
        assert_eq!(super::status_counts(td.path()), porcelain_counts(td.path()));
    }

    #[test]
    fn one_untracked_file() {
        let td = fixtures::repo();
        fixtures::write(td.path(), "a.txt", "x");
        assert_eq!(super::status_counts(td.path()), porcelain_counts(td.path()));
    }

    #[test]
    fn one_modified_tracked_file() {
        let td = fixtures::repo();
        fixtures::write(td.path(), "t.txt", "initial");
        fixtures::git(td.path(), &["add", "t.txt"]);
        fixtures::git(td.path(), &["commit", "-q", "-m", "Add t.txt"]);
        fixtures::write(td.path(), "t.txt", "modified");
        assert_eq!(super::status_counts(td.path()), porcelain_counts(td.path()));
    }

    #[test]
    fn one_staged_new_file_and_one_untracked_file() {
        let td = fixtures::repo();
        fixtures::write(td.path(), "a.txt", "x");
        fixtures::git(td.path(), &["add", "a.txt"]);
        fixtures::write(td.path(), "b.txt", "y");
        assert_eq!(super::status_counts(td.path()), porcelain_counts(td.path()));
    }
}
