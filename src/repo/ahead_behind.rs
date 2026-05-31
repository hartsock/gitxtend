use crate::error::GitxtendError;
use crate::repo::Result;
use std::collections::HashSet;
use std::path::Path;

/// (ahead, behind) commit counts of HEAD relative to `upstream`:
///   ahead  = commits reachable from HEAD but not `upstream`  (git: upstream..HEAD)
///   behind = commits reachable from `upstream` but not HEAD   (git: HEAD..upstream)
/// Returns (0, 0) if either revision can't be resolved.
pub fn ahead_behind(path: &Path, upstream: &str) -> Result<(usize, usize)> {
    let repo = gix::open(path).map_err(GitxtendError::from_err)?;
    let head = match repo.rev_parse_single("HEAD") {
        Ok(id) => id.detach(),
        Err(_) => return Ok((0, 0)),
    };
    let up = match repo.rev_parse_single(upstream) {
        Ok(id) => id.detach(),
        Err(_) => return Ok((0, 0)),
    };
    // gix 0.70's rev-walk has no built-in exclusion, so collect each side's
    // full reachable set and diff them: ahead = HEAD-only commits
    // (git: upstream..HEAD), behind = upstream-only commits (git: HEAD..upstream).
    let reachable = |tip: gix::ObjectId| -> Result<HashSet<gix::ObjectId>> {
        Ok(repo
            .rev_walk([tip])
            .all()
            .map_err(GitxtendError::from_err)?
            .filter_map(|info| info.ok())
            .map(|info| info.id)
            .collect())
    };
    let head_set = reachable(head)?;
    let up_set = reachable(up)?;
    let ahead = head_set.difference(&up_set).count();
    let behind = up_set.difference(&head_set).count();
    Ok((ahead, behind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures;

    #[test]
    fn ahead_behind_initial() {
        let td = fixtures::repo();
        assert_eq!(ahead_behind(td.path(), "origin/main").unwrap(), (0, 0));
    }

    #[test]
    fn ahead_behind_local_commits() {
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

        // Assert ahead == 2, behind == 0
        assert_eq!(ahead_behind(p, "origin/main").unwrap(), (2, 0));

        // Parity assertion against git rev-list --count
        let a: usize = fixtures::git(p, &["rev-list", "--count", "origin/main..HEAD"])
            .parse()
            .unwrap();
        let b: usize = fixtures::git(p, &["rev-list", "--count", "HEAD..origin/main"])
            .parse()
            .unwrap();
        assert_eq!(ahead_behind(p, "origin/main").unwrap(), (a, b));
    }

    #[test]
    fn ahead_behind_unresolvable_upstream() {
        let td = fixtures::repo();
        assert_eq!(
            ahead_behind(td.path(), "unresolvable-upstream").unwrap(),
            (0, 0)
        );
    }
}
