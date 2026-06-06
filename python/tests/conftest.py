from __future__ import annotations

from unittest.mock import MagicMock

import pytest


PUBLIC_EXTENSION_NAMES = (
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
    "pull",
    "push",
    "add",
    "commit",
    "stash_push",
    "stash_pop",
    "create_branch",
    "reset_hard",
    "rebase",
    "stash_rebase",
)


@pytest.fixture
def mock_gitxtend(monkeypatch):
    """Patch the compiled extension surface for Python unit tests."""
    import gitxtend
    import gitxtend._gitxtend as extension

    mock_extension = MagicMock(name="gitxtend._gitxtend")

    for name in PUBLIC_EXTENSION_NAMES:
        mock = MagicMock(name=f"gitxtend._gitxtend.{name}")
        setattr(mock_extension, name, mock)
        monkeypatch.setattr(extension, name, mock, raising=False)
        monkeypatch.setattr(gitxtend, name, mock, raising=False)

    return mock_extension
