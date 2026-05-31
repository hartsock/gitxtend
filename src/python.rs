//! PyO3 surface — compiled ONLY with the `python` feature. Thin wrappers that
//! call into the pure-Rust `repo` / `status` cores and convert types/errors for
//! Python. Each M1 method task replaces that method's `todo!()` wrapper with a
//! call into `crate::repo::<method>` (soft-fail methods map errors to the
//! sentinel; the rest propagate `GitxtendError` → `PyRuntimeError`).
//!
//! Soft-fail methods (see `docs/API.md`) must return sentinels (None/0/[]/{})
//! instead of raising, to stay drop-in compatible with git-tend's GitService.

use pyo3::prelude::*;
use std::collections::HashMap;

/// Roll-up mirroring `StatusService.check_repo` / `models.RepoStatus`.
///
/// `skip_from_py_object`: this type is returned to Python, never parsed from it,
/// so we opt out of the (now-opt-in) `FromPyObject` derive.
#[pyclass(skip_from_py_object)]
#[derive(Clone, Default)]
pub struct RepoStatus {
    #[pyo3(get)]
    pub path: String,
    #[pyo3(get)]
    pub state: String, // SyncState value, see docs/API.md
    #[pyo3(get)]
    pub local_branch: Option<String>,
    #[pyo3(get)]
    pub tracking_branch: Option<String>,
    #[pyo3(get)]
    pub local_sha: Option<String>,
    #[pyo3(get)]
    pub remote_sha: Option<String>,
    #[pyo3(get)]
    pub ahead_count: usize,
    #[pyo3(get)]
    pub behind_count: usize,
    #[pyo3(get)]
    pub new_remote_commits: Vec<String>,
    #[pyo3(get)]
    pub is_dirty: bool,
    #[pyo3(get)]
    pub error: Option<String>,
}

// ---- Read primitives — Python wrappers (todo! until each method lands) ----
//
// As each `crate::repo::<method>` lands, replace the matching `todo!()` body
// with a call into it. The pure-Rust core is what carries the gix logic + tests.

#[pyfunction]
fn is_git_repo(_path: String) -> PyResult<bool> {
    todo!("repo::is_git_repo (gix::discover)")
}

#[pyfunction]
fn is_clean(_path: String) -> PyResult<bool> {
    todo!("repo::is_clean (gix status, empty)")
}

#[pyfunction]
fn current_branch(_path: String) -> PyResult<Option<String>> {
    todo!("repo::current_branch (None if detached)")
}

#[pyfunction]
fn tracking_branch(_path: String) -> PyResult<Option<String>> {
    todo!("repo::tracking_branch (configured upstream)")
}

#[pyfunction]
fn head_sha(_path: String) -> PyResult<Option<String>> {
    todo!("repo::head_sha (repo.head_id)")
}

#[pyfunction]
fn remote_head_sha(_path: String, _remote_ref: String) -> PyResult<Option<String>> {
    todo!("repo::remote_head_sha (resolve remote-tracking ref)")
}

#[pyfunction]
fn ahead_behind(_path: String, _upstream: String) -> PyResult<(usize, usize)> {
    todo!("repo::ahead_behind (single graph walk)")
}

#[pyfunction]
fn rev_list_count(_path: String, _range_spec: String) -> PyResult<usize> {
    todo!("repo::rev_list_count (soft-fail 0)")
}

#[pyfunction]
fn log_subjects(_path: String, _range_spec: String, _max_count: usize) -> PyResult<Vec<String>> {
    todo!("repo::log_subjects (summaries, newest first)")
}

#[pyfunction]
fn remote_urls(_path: String) -> PyResult<HashMap<String, String>> {
    todo!("repo::remote_urls (name -> fetch url)")
}

#[pyfunction]
fn last_commit_date(_path: String) -> PyResult<Option<String>> {
    todo!("repo::last_commit_date (ISO 8601 %aI)")
}

#[pyfunction]
fn status_counts(_path: String) -> PyResult<(usize, usize)> {
    todo!("repo::status_counts (modified, untracked)")
}

#[pyfunction]
fn fetch(_path: String, _remote: Option<String>) -> PyResult<bool> {
    todo!("repo::fetch (gix fetch, contained shell-out fallback)")
}

#[pyfunction]
fn repo_status(_path: String, _fetch: bool) -> PyResult<RepoStatus> {
    todo!("status::repo_status (full SyncState decision tree)")
}

#[pymodule]
fn _gitxtend(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RepoStatus>()?;
    m.add_function(wrap_pyfunction!(is_git_repo, m)?)?;
    m.add_function(wrap_pyfunction!(is_clean, m)?)?;
    m.add_function(wrap_pyfunction!(current_branch, m)?)?;
    m.add_function(wrap_pyfunction!(tracking_branch, m)?)?;
    m.add_function(wrap_pyfunction!(head_sha, m)?)?;
    m.add_function(wrap_pyfunction!(remote_head_sha, m)?)?;
    m.add_function(wrap_pyfunction!(ahead_behind, m)?)?;
    m.add_function(wrap_pyfunction!(rev_list_count, m)?)?;
    m.add_function(wrap_pyfunction!(log_subjects, m)?)?;
    m.add_function(wrap_pyfunction!(remote_urls, m)?)?;
    m.add_function(wrap_pyfunction!(last_commit_date, m)?)?;
    m.add_function(wrap_pyfunction!(status_counts, m)?)?;
    m.add_function(wrap_pyfunction!(fetch, m)?)?;
    m.add_function(wrap_pyfunction!(repo_status, m)?)?;
    Ok(())
}
