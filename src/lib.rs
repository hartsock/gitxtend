//! gitxtend — gitoxide-backed git repository tending, exposed to Python.
//!
//! This file is a SCAFFOLD. It declares the Python-visible surface defined in
//! `docs/API.md` so the module compiles and imports on gnuc; every function
//! body is `todo!()`. Implement them per `docs/PORTING.md`, keeping the pure
//! gix logic in `repo.rs` / `status.rs` (PyO3-free, unit-testable) and using
//! this file only for Python type/error conversion.
//!
//! Soft-fail methods (see API.md) must return sentinels (None/0/[]/{}) instead
//! of raising, to stay drop-in compatible with git-tend's GitService.

use pyo3::prelude::*;

// Pure-Rust cores (to be implemented). Keep PyO3 out of these so they can be
// unit-tested with gix fixtures and reused by an optional CLI bin target.
// mod repo;
// mod status;

/// Roll-up mirroring `StatusService.check_repo` / `models.RepoStatus`.
#[pyclass]
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

// ---- Read primitives (port of GitService read side) ---------------------

#[pyfunction]
fn is_git_repo(_path: String) -> PyResult<bool> {
    todo!("PORTING.md → is_git_repo (gix::discover)")
}

#[pyfunction]
fn is_clean(_path: String) -> PyResult<bool> {
    todo!("PORTING.md → is_clean (gix status, empty)")
}

#[pyfunction]
fn current_branch(_path: String) -> PyResult<Option<String>> {
    todo!("PORTING.md → current_branch (None if detached)")
}

#[pyfunction]
fn tracking_branch(_path: String) -> PyResult<Option<String>> {
    todo!("PORTING.md → tracking_branch (configured upstream)")
}

#[pyfunction]
fn head_sha(_path: String) -> PyResult<Option<String>> {
    todo!("PORTING.md → head_sha (repo.head_id)")
}

#[pyfunction]
#[pyo3(signature = (path, remote_ref="origin/main".to_string()))]
fn remote_head_sha(_path: String, _remote_ref: String) -> PyResult<Option<String>> {
    todo!("PORTING.md → remote_head_sha (resolve remote-tracking ref)")
}

#[pyfunction]
fn ahead_behind(_path: String, _upstream: String) -> PyResult<(usize, usize)> {
    todo!("PORTING.md → ahead_behind (single graph walk)")
}

#[pyfunction]
fn rev_list_count(_path: String, _range_spec: String) -> PyResult<usize> {
    todo!("PORTING.md → rev_list_count (soft-fail 0)")
}

#[pyfunction]
#[pyo3(signature = (path, range_spec, max_count=10))]
fn log_subjects(_path: String, _range_spec: String, _max_count: usize) -> PyResult<Vec<String>> {
    todo!("PORTING.md → log_subjects (summaries, newest first)")
}

#[pyfunction]
fn remote_urls(_path: String) -> PyResult<std::collections::HashMap<String, String>> {
    todo!("PORTING.md → remote_urls (name -> fetch url)")
}

#[pyfunction]
fn last_commit_date(_path: String) -> PyResult<Option<String>> {
    todo!("PORTING.md → last_commit_date (ISO 8601 %aI)")
}

#[pyfunction]
fn status_counts(_path: String) -> PyResult<(usize, usize)> {
    todo!("PORTING.md → status_counts (modified, untracked)")
}

#[pyfunction]
#[pyo3(signature = (path, remote=None))]
fn fetch(_path: String, _remote: Option<String>) -> PyResult<bool> {
    todo!("PORTING.md → fetch (gix fetch, contained shell-out fallback)")
}

// ---- Roll-up (port of StatusService.check_repo) -------------------------

#[pyfunction]
#[pyo3(signature = (path, fetch=true))]
fn repo_status(_path: String, _fetch: bool) -> PyResult<RepoStatus> {
    todo!("PORTING.md → repo_status (full SyncState decision tree)")
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
