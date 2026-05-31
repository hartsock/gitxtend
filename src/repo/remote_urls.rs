use std::collections::HashMap;
use std::path::Path;

/// Map of remote name -> fetch URL, like the `(fetch)` lines of
/// `git remote -v`. Soft-fails to an empty map on any error.
pub fn remote_urls(path: &Path) -> HashMap<String, String> {
    let Ok(repo) = gix::open(path) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for name in repo.remote_names() {
        if let Ok(remote) = repo.find_remote(name.as_ref()) {
            if let Some(url) = remote.url(gix::remote::Direction::Fetch) {
                out.insert(name.to_string(), url.to_bstring().to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures;

    #[test]
    fn no_remotes() {
        let td = fixtures::repo();
        assert!(remote_urls(td.path()).is_empty());
    }

    #[test]
    fn one_remote() {
        let td = fixtures::repo();
        fixtures::git(
            td.path(),
            &["remote", "add", "origin", "https://example.com/r.git"],
        );
        let urls = remote_urls(td.path());
        assert_eq!(urls.len(), 1);
        assert_eq!(
            urls.get("origin"),
            Some(&"https://example.com/r.git".to_string())
        );
        assert_eq!(
            fixtures::git(td.path(), &["config", "remote.origin.url"]),
            "https://example.com/r.git"
        );
    }

    #[test]
    fn two_remotes() {
        let td = fixtures::repo();
        fixtures::git(
            td.path(),
            &["remote", "add", "origin", "https://example.com/r.git"],
        );
        fixtures::git(
            td.path(),
            &["remote", "add", "upstream", "https://example.com/u.git"],
        );
        let urls = remote_urls(td.path());
        assert_eq!(urls.len(), 2);
        assert_eq!(
            urls.get("origin"),
            Some(&"https://example.com/r.git".to_string())
        );
        assert_eq!(
            urls.get("upstream"),
            Some(&"https://example.com/u.git".to_string())
        );
    }
}
