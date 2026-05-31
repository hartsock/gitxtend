//! gitxtend — gitoxide-backed git repository tending, exposed to Python.
//!
//! Crate layout:
//! - [`error`]  — [`GitxtendError`] + the crate [`Result`] alias.
//! - [`repo`]   — pure-Rust, gix-backed read primitives (one module per method).
//! - [`status`] — the `repo_status` roll-up + SyncState logic.
//! - `python`   — the PyO3 surface, compiled ONLY with the `python` feature.
//!
//! The pure-Rust core (`repo`, `status`) carries NO PyO3, so it unit-tests with
//! gix fixtures and `cargo test` without a Python interpreter. Build the Python
//! wheel with `maturin` (which enables `python` + `extension-module`). See
//! `docs/DESIGN.md` / `docs/PORTING.md`.

pub mod error;
pub mod repo;
pub mod status;

pub use error::{GitxtendError, Result};

#[cfg(feature = "python")]
mod python;
