from __future__ import annotations

import gitxtend
from conftest import PLANNED_WRITE_EXTENSION_NAMES


def test_mock_gitxtend_fixture_replaces_extension_exports(mock_gitxtend):
    mock_gitxtend.is_git_repo.return_value = True

    assert gitxtend.is_git_repo("/tmp/repo") is True
    mock_gitxtend.is_git_repo.assert_called_once_with("/tmp/repo")


def test_mock_gitxtend_fixture_includes_write_side_exports(mock_gitxtend):
    write_side_calls = {
        "pull": ("/tmp/repo",),
        "push": ("/tmp/repo", "origin", "main"),
        "add": ("/tmp/repo", ["README.md"]),
        "commit": ("/tmp/repo", "initial import"),
        "stash_push": ("/tmp/repo", "before-rebase"),
        "stash_pop": ("/tmp/repo",),
        "create_branch": ("/tmp/repo", "feature/test"),
        "reset_hard": ("/tmp/repo", "HEAD~1"),
        "rebase": ("/tmp/repo", "origin/main"),
        "stash_rebase": ("/tmp/repo", "origin/main"),
    }

    assert set(write_side_calls) == set(PLANNED_WRITE_EXTENSION_NAMES)

    for name, args in write_side_calls.items():
        getattr(mock_gitxtend, name).return_value = name

        assert getattr(gitxtend, name)(*args) == name
        getattr(mock_gitxtend, name).assert_called_once_with(*args)
