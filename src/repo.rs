//! Pure-Rust, gix-backed read primitives. NO PyO3 here — keep this module
//! testable with gix fixtures and reusable by an optional CLI bin target.
//!
//! Implement each function per docs/PORTING.md. The PyO3 layer in lib.rs is a
//! thin wrapper that calls into here and converts errors/types for Python.
//!
//! Suggested signatures (adjust to your gix version):
//!
//!   pub fn is_git_repo(path: &Path) -> bool
//!   pub fn is_clean(path: &Path) -> Result<bool>
//!   pub fn current_branch(path: &Path) -> Result<Option<String>>
//!   pub fn tracking_branch(path: &Path) -> Result<Option<String>>
//!   pub fn head_sha(path: &Path) -> Result<Option<String>>
//!   pub fn remote_head_sha(path: &Path, remote_ref: &str) -> Result<Option<String>>
//!   pub fn ahead_behind(path: &Path, upstream: &str) -> Result<(usize, usize)>
//!   pub fn rev_list_count(path: &Path, range_spec: &str) -> usize  // soft-fail 0
//!   pub fn log_subjects(path: &Path, range_spec: &str, max: usize) -> Vec<String>
//!   pub fn remote_urls(path: &Path) -> HashMap<String, String>
//!   pub fn last_commit_date(path: &Path) -> Result<Option<String>>
//!   pub fn status_counts(path: &Path) -> (usize, usize)  // (modified, untracked)
//!   pub fn fetch(path: &Path, remote: Option<&str>) -> Result<bool>

// TODO(M1): implement per docs/PORTING.md, with parity tests vs the git CLI.

#[cfg(test)]
mod tests {
    // TODO(M1): temp-dir gix fixtures; assert parity with `git` for every
    // method and every SyncState in the decision tree (see docs/PORTING.md).
}
