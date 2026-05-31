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

/// Roll-up mirroring `check_repo` / `RepoStatus`.
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
fn is_git_repo(path: String) -> PyResult<bool> {
    Ok(crate::repo::is_git_repo(std::path::Path::new(&path)))
}

#[pyfunction]
fn is_clean(path: String) -> PyResult<bool> {
    Ok(crate::repo::is_clean(std::path::Path::new(&path))?)
}

#[pyfunction]
fn current_branch(path: String) -> PyResult<Option<String>> {
    // soft-fail: any error -> None (API.md)
    Ok(crate::repo::current_branch(std::path::Path::new(&path)).unwrap_or(None))
}

#[pyfunction]
fn tracking_branch(path: String) -> PyResult<Option<String>> {
    // soft-fail: any error -> None (API.md)
    Ok(crate::repo::tracking_branch(std::path::Path::new(&path)).unwrap_or(None))
}

#[pyfunction]
fn head_sha(path: String) -> PyResult<Option<String>> {
    // soft-fail: any error -> None (API.md)
    Ok(crate::repo::head_sha(std::path::Path::new(&path)).unwrap_or(None))
}

#[pyfunction]
#[pyo3(signature = (path, remote_ref="origin/main".to_string()))]
fn remote_head_sha(path: String, remote_ref: String) -> PyResult<Option<String>> {
    // soft-fail: any error -> None (API.md)
    Ok(crate::repo::remote_head_sha(std::path::Path::new(&path), &remote_ref).unwrap_or(None))
}

#[pyfunction]
fn ahead_behind(path: String, upstream: String) -> PyResult<(usize, usize)> {
    // soft-fail: unresolvable revs -> (0, 0)
    Ok(crate::repo::ahead_behind(std::path::Path::new(&path), &upstream).unwrap_or((0, 0)))
}

#[pyfunction]
fn rev_list_count(path: String, range_spec: String) -> PyResult<usize> {
    Ok(crate::repo::rev_list_count(
        std::path::Path::new(&path),
        &range_spec,
    ))
}

#[pyfunction]
#[pyo3(signature = (path, range_spec, max_count=10))]
fn log_subjects(path: String, range_spec: String, max_count: usize) -> PyResult<Vec<String>> {
    Ok(crate::repo::log_subjects(
        std::path::Path::new(&path),
        &range_spec,
        max_count,
    ))
}

#[pyfunction]
fn remote_urls(path: String) -> PyResult<HashMap<String, String>> {
    Ok(crate::repo::remote_urls(std::path::Path::new(&path)))
}

#[pyfunction]
fn last_commit_date(_path: String) -> PyResult<Option<String>> {
    todo!("repo::last_commit_date (ISO 8601 %aI)")
}

#[pyfunction]
fn status_counts(path: String) -> PyResult<(usize, usize)> {
    Ok(crate::repo::status_counts(std::path::Path::new(&path)))
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
