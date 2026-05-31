"""gitxtend — gitoxide-backed git repository tending.

The implementation lives in the compiled Rust extension ``gitxtend._gitxtend``
(built by maturin). This package re-exports its public surface so callers do
``import gitxtend`` / ``from gitxtend import repo_status``.

See docs/API.md for the contract.
"""

from __future__ import annotations

from ._gitxtend import (  # type: ignore[attr-defined]
    RepoStatus,
    ahead_behind,
    current_branch,
    fetch,
    head_sha,
    is_clean,
    is_git_repo,
    last_commit_date,
    log_subjects,
    remote_head_sha,
    remote_urls,
    repo_status,
    rev_list_count,
    status_counts,
    tracking_branch,
)

__all__ = [
    "RepoStatus",
    "ahead_behind",
    "current_branch",
    "fetch",
    "head_sha",
    "is_clean",
    "is_git_repo",
    "last_commit_date",
    "log_subjects",
    "remote_head_sha",
    "remote_urls",
    "repo_status",
    "rev_list_count",
    "status_counts",
    "tracking_branch",
]
