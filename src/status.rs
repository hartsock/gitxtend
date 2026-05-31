//! Pure-Rust roll-up mirroring `StatusService.check_repo`. NO PyO3 here.
//!
//! Implement `repo_status(path, fetch) -> RepoStatusData` following the exact
//! sequence and SyncState decision tree in docs/PORTING.md / docs/API.md:
//!
//! ```text
//! 1. not a repo            -> state="error", error set
//! 2. no upstream           -> state="no-remote", is_dirty filled
//! 3. fetch (if requested)
//! 4. local/remote sha, ahead/behind, new_remote_commits when behind>0
//! 5. is_dirty; then:
//!      ahead>0 && behind>0 -> "diverged"
//!      ahead>0             -> "ahead"
//!      behind>0            -> "behind"
//!      is_dirty            -> "dirty"
//!      else                -> "up-to-date"
//! ```
//!
//! Return a plain Rust struct; lib.rs converts it to the #[pyclass] RepoStatus.

// TODO(M1): implement using repo.rs primitives.

#[cfg(test)]
mod tests {
    // TODO(M1): fixtures for diverged/ahead/behind/dirty/no-remote/error.
}
