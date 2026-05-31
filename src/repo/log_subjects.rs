use std::collections::HashSet;
use std::path::Path;

/// Commit summaries (first line of each message), NEWEST-FIRST, for the
/// commits in `range_spec`, capped at `max_count`. Mirrors
/// `git log --format=%s --max-count=N <range_spec>`. Two-dot "A..B" =
/// commits reachable from B but not A; a single rev = its ancestors.
/// Soft-fails to `Vec::new()` on any error.
pub fn log_subjects(path: &Path, range_spec: &str, max_count: usize) -> Vec<String> {
    let Ok(repo) = gix::open(path) else {
        return Vec::new();
    };
    let reachable = |rev: &str| -> Option<HashSet<gix::ObjectId>> {
        let id = repo.rev_parse_single(rev).ok()?.detach();
        Some(
            repo.rev_walk([id])
                .all()
                .ok()?
                .filter_map(|i| i.ok())
                .map(|i| i.id)
                .collect(),
        )
    };
    let (tip, exclude) = if let Some((a, b)) = range_spec.split_once("..") {
        match (repo.rev_parse_single(b).ok(), reachable(a)) {
            (Some(bid), Some(aset)) => (bid.detach(), aset),
            _ => return Vec::new(),
        }
    } else {
        match repo.rev_parse_single(range_spec).ok() {
            Some(id) => (id.detach(), HashSet::new()),
            None => return Vec::new(),
        }
    };
    let walk = match repo
        .rev_walk([tip])
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
    {
        Ok(w) => w,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for info in walk {
        let Ok(info) = info else { continue };
        if exclude.contains(&info.id) {
            continue;
        }
        if let Ok(obj) = repo.find_object(info.id) {
            if let Ok(commit) = obj.try_into_commit() {
                if let Ok(msg) = commit.message() {
                    out.push(msg.summary().to_string());
                }
            }
        }
        if out.len() >= max_count {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures;

    #[test]
    fn test_log_subjects_with_three_commits() {
        let td = fixtures::repo();
        let p = td.path();
        fixtures::write(p, "file1.txt", "content1");
        fixtures::git(p, &["add", "file1.txt"]);
        fixtures::git(p, &["commit", "-m", "one"]);

        fixtures::write(p, "file2.txt", "content2");
        fixtures::git(p, &["add", "file2.txt"]);
        fixtures::git(p, &["commit", "-m", "two"]);

        fixtures::write(p, "file3.txt", "content3");
        fixtures::git(p, &["add", "file3.txt"]);
        fixtures::git(p, &["commit", "-m", "three"]);

        let result = log_subjects(p, "HEAD", 10);
        assert_eq!(result, vec!["three", "two", "one", "init"]);
    }

    #[test]
    fn test_log_subjects_with_max_count() {
        let td = fixtures::repo();
        let p = td.path();
        fixtures::write(p, "file1.txt", "content1");
        fixtures::git(p, &["add", "file1.txt"]);
        fixtures::git(p, &["commit", "-m", "one"]);

        fixtures::write(p, "file2.txt", "content2");
        fixtures::git(p, &["add", "file2.txt"]);
        fixtures::git(p, &["commit", "-m", "two"]);

        let result = log_subjects(p, "HEAD", 2);
        assert_eq!(result.len(), 2);
        assert_eq!(result, vec!["two", "one"]);
    }

    #[test]
    fn test_log_subjects_with_two_dot_range() {
        let td = fixtures::repo();
        let p = td.path();

        // Create a bare remote repository
        let remote_td = tempfile::tempdir().expect("tempdir");
        let remote_path = remote_td.path();
        fixtures::git(remote_path, &["init", "--bare"]);

        // Add the remote to the local repository
        fixtures::git(
            p,
            &["remote", "add", "origin", remote_path.to_str().unwrap()],
        );

        // Push initial commit to the remote
        fixtures::git(p, &["push", "-u", "origin", "main"]);

        // Create two more commits locally
        fixtures::write(p, "file4.txt", "content4");
        fixtures::git(p, &["add", "file4.txt"]);
        fixtures::git(p, &["commit", "-m", "four"]);

        fixtures::write(p, "file5.txt", "content5");
        fixtures::git(p, &["add", "file5.txt"]);
        fixtures::git(p, &["commit", "-m", "five"]);

        let result = log_subjects(p, "origin/main..HEAD", 10);
        assert_eq!(result, vec!["five", "four"]);
    }

    #[test]
    fn test_log_subjects_with_invalid_range() {
        let td = fixtures::repo();
        let p = td.path();
        let result = log_subjects(p, "nope..HEAD", 10);
        assert_eq!(result, Vec::<String>::new());
    }
}
