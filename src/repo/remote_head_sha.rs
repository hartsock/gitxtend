use crate::error::GitxtendError;
use crate::repo::Result;
use std::path::Path;

/// Object id (full hex) that a remote-tracking ref resolves to, e.g.
/// "origin/main"; `Ok(None)` if that ref doesn't exist. Mirrors
/// `git rev-parse <remote_ref>`.
pub fn remote_head_sha(path: &Path, remote_ref: &str) -> Result<Option<String>> {
    let repo = gix::open(path).map_err(GitxtendError::from_err)?;
    match repo.rev_parse_single(remote_ref) {
        Ok(id) => Ok(Some(id.detach().to_string())),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures;
    use tempfile::tempdir;

    #[test]
    fn no_such_remote_ref() {
        let td = fixtures::repo();
        let p = td.path();
        assert_eq!(remote_head_sha(p, "origin/main").unwrap(), None);
    }

    #[test]
    fn with_a_remote() {
        let remote = tempdir().unwrap();
        fixtures::git(remote.path(), &["init", "--bare", "-q", "-b", "main"]);
        let td = fixtures::repo();
        let p = td.path();
        fixtures::git(
            p,
            &["remote", "add", "origin", &remote.path().to_string_lossy()],
        );
        fixtures::write(p, "file.txt", "content");
        fixtures::git(p, &["add", "-A"]);
        fixtures::git(p, &["commit", "-m", "initial commit"]);
        fixtures::git(p, &["push", "-q", "-u", "origin", "main"]);

        let expected = fixtures::git(p, &["rev-parse", "origin/main"]);
        assert_eq!(remote_head_sha(p, "origin/main").unwrap(), Some(expected));
    }
}
