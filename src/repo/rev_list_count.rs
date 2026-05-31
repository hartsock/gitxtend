use std::collections::HashSet;
use std::path::Path;

/// Count commits in a revision range, like `git rev-list --count <range_spec>`.
/// Supports a two-dot range "A..B" (commits reachable from B but not A) and a
/// single revision "X" (all commits reachable from X). Soft-fails to 0 on any
/// parse/lookup/open error, matching the tool's current behaviour.
pub fn rev_list_count(path: &Path, range_spec: &str) -> usize {
    let Ok(repo) = gix::open(path) else { return 0 };
    let reachable = |rev: &str| -> Option<HashSet<gix::ObjectId>> {
        let id = repo.rev_parse_single(rev).ok()?.detach();
        Some(
            repo.rev_walk([id])
                .all()
                .ok()?
                .filter_map(|info| info.ok())
                .map(|info| info.id)
                .collect(),
        )
    };
    if let Some((a, b)) = range_spec.split_once("..") {
        match (reachable(a), reachable(b)) {
            (Some(sa), Some(sb)) => sb.difference(&sa).count(),
            _ => 0,
        }
    } else {
        reachable(range_spec).map(|s| s.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures;

    #[test]
    fn single_rev_count() {
        let td = fixtures::repo();
        let p = td.path();

        // Add 2 more commits
        fixtures::write(p, "file1.txt", "content1");
        fixtures::git(p, &["add", "file1.txt"]);
        fixtures::git(p, &["commit", "-q", "-m", "second commit"]);
        fixtures::write(p, "file2.txt", "content2");
        fixtures::git(p, &["add", "file2.txt"]);
        fixtures::git(p, &["commit", "-q", "-m", "third commit"]);

        // Assert count is 3
        assert_eq!(rev_list_count(p, "HEAD"), 3);

        // Parity assertion against git rev-list --count
        let expected: usize = fixtures::git(p, &["rev-list", "--count", "HEAD"])
            .parse()
            .unwrap();
        assert_eq!(rev_list_count(p, "HEAD"), expected);
    }

    #[test]
    fn two_dot_range_count() {
        let td = fixtures::repo();
        let p = td.path();

        // Set up a real bare remote and push main so origin/main == HEAD
        let remote = tempfile::tempdir().unwrap();
        fixtures::git(remote.path(), &["init", "--bare", "-q", "-b", "main"]);
        fixtures::git(
            p,
            &["remote", "add", "origin", &remote.path().to_string_lossy()],
        );
        fixtures::git(p, &["push", "-q", "-u", "origin", "main"]);

        // Add 2 local commits without pushing
        fixtures::write(p, "file1.txt", "content1");
        fixtures::git(p, &["add", "file1.txt"]);
        fixtures::git(p, &["commit", "-q", "-m", "second commit"]);
        fixtures::write(p, "file2.txt", "content2");
        fixtures::git(p, &["add", "file2.txt"]);
        fixtures::git(p, &["commit", "-q", "-m", "third commit"]);

        // Assert count is 2
        assert_eq!(rev_list_count(p, "origin/main..HEAD"), 2);

        // Parity assertion against git rev-list --count
        let expected: usize = fixtures::git(p, &["rev-list", "--count", "origin/main..HEAD"])
            .parse()
            .unwrap();
        assert_eq!(rev_list_count(p, "origin/main..HEAD"), expected);
    }

    #[test]
    fn soft_fail_on_nonexistent_rev() {
        let td = fixtures::repo();
        let p = td.path();

        // Assert count is 0 for non-existent rev
        assert_eq!(rev_list_count(p, "nonexistent-rev"), 0);
    }
}
