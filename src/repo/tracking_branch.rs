use crate::error::GitxtendError;
use crate::repo::Result;
use std::path::Path;

/// The configured upstream of the current branch as a short "remote/branch"
/// name (e.g. "origin/main"); `Ok(None)` if there is no upstream or HEAD is
/// detached. Mirrors `git rev-parse --abbrev-ref @{upstream}`.
pub fn tracking_branch(path: &Path) -> Result<Option<String>> {
    let repo = gix::open(path).map_err(GitxtendError::from_err)?;
    let head = repo.head().map_err(GitxtendError::from_err)?;
    let branch = match head.referent_name() {
        Some(n) => n.shorten().to_string(), // e.g. "main"
        None => return Ok(None),            // detached
    };
    let cfg = repo.config_snapshot();
    // branch.<name>.remote  -> e.g. "origin"   ;  branch.<name>.merge -> "refs/heads/main"
    let remote_key = format!("branch.{branch}.remote");
    let merge_key = format!("branch.{branch}.merge");
    let remote = cfg.string(remote_key.as_str()).map(|v| v.to_string());
    let merge = cfg.string(merge_key.as_str()).map(|v| v.to_string());
    match (remote, merge) {
        (Some(r), Some(m)) => {
            let short = m.strip_prefix("refs/heads/").unwrap_or(&m);
            Ok(Some(format!("{r}/{short}")))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures::{git, repo};

    #[test]
    fn no_upstream() {
        let td = repo();
        assert_eq!(tracking_branch(td.path()).unwrap(), None);
    }

    #[test]
    fn with_upstream() {
        let td = repo();
        let remote = tempfile::tempdir().unwrap();
        git(remote.path(), &["init", "--bare", "-q", "-b", "main"]);
        git(
            td.path(),
            &["remote", "add", "origin", &remote.path().to_string_lossy()],
        );
        git(td.path(), &["push", "-q", "-u", "origin", "main"]);

        let expected = Some("origin/main".into());
        assert_eq!(tracking_branch(td.path()).unwrap(), expected);
        assert_eq!(
            tracking_branch(td.path()).unwrap(),
            Some(git(
                td.path(),
                &["rev-parse", "--abbrev-ref", "@{upstream}"]
            ))
        );
    }
}
