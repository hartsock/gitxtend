use crate::error::GitxtendError;
use crate::repo::Result;
use std::path::Path;

/// HEAD commit's full hex object id; `Ok(None)` on an unborn/empty repo
/// (mirrors `git rev-parse HEAD`).
pub fn head_sha(path: &Path) -> Result<Option<String>> {
    let repo = gix::open(path).map_err(GitxtendError::from_err)?;
    Ok(repo
        .head()
        .map_err(GitxtendError::from_err)?
        .id()
        .map(|id| id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures;

    #[test]
    fn head_sha_matches_git_rev_parse() {
        let td = fixtures::repo();
        let p = td.path();
        let expected = fixtures::git(p, &["rev-parse", "HEAD"]);
        assert_eq!(head_sha(p).unwrap(), Some(expected));

        fixtures::write(p, "file.txt", "content");
        fixtures::git(p, &["add", "-A"]);
        fixtures::git(p, &["commit", "-m", "two"]);

        let expected = fixtures::git(p, &["rev-parse", "HEAD"]);
        assert_eq!(head_sha(p).unwrap(), Some(expected));
    }
}
