from __future__ import annotations

from unittest.mock import MagicMock

import pytest


PLANNED_WRITE_EXTENSION_NAMES = (
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
    public_extension_names = (*gitxtend.__all__, *PLANNED_WRITE_EXTENSION_NAMES)

    for name in public_extension_names:
        mock = MagicMock(name=f"gitxtend._gitxtend.{name}")
        setattr(mock_extension, name, mock)
        monkeypatch.setattr(extension, name, mock, raising=False)
        monkeypatch.setattr(gitxtend, name, mock, raising=False)

    return mock_extension
