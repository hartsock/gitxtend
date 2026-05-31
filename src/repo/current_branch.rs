use crate::error::GitxtendError;
use crate::repo::Result;
use std::path::Path;

/// Short name of the current branch (e.g. "main"); `Ok(None)` when HEAD is
/// detached (mirrors `git rev-parse --abbrev-ref HEAD`, which prints "HEAD"
/// when detached — we return None in that case).
pub fn current_branch(path: &Path) -> Result<Option<String>> {
    let repo = gix::open(path).map_err(GitxtendError::from_err)?;
    let head = repo.head().map_err(GitxtendError::from_err)?;
    Ok(head.referent_name().map(|n| n.shorten().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures;

    #[test]
    fn test_current_branch_on_main() {
        let td = fixtures::repo();
        let p = td.path();
        assert_eq!(current_branch(p).unwrap(), Some("main".into()));
        assert_eq!(
            current_branch(p).unwrap(),
            Some(fixtures::git(p, &["rev-parse", "--abbrev-ref", "HEAD"]))
        );
    }

    #[test]
    fn test_current_branch_detached_head() {
        let td = fixtures::repo();
        let p = td.path();
        let sha = fixtures::git(p, &["rev-parse", "HEAD"]);
        fixtures::git(p, &["checkout", "--detach", &sha]);
        assert_eq!(current_branch(p).unwrap(), None);
    }
}
