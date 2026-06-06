from __future__ import annotations

import gitxtend


def test_mock_gitxtend_fixture_replaces_extension_exports(mock_gitxtend):
    mock_gitxtend.is_git_repo.return_value = True

    assert gitxtend.is_git_repo("/tmp/repo") is True
    mock_gitxtend.is_git_repo.assert_called_once_with("/tmp/repo")


def test_mock_gitxtend_fixture_includes_write_side_exports(mock_gitxtend):
    for name in (
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
    ):
        assert hasattr(mock_gitxtend, name)
