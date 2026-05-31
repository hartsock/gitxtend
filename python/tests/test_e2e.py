"""End-to-end tests for the compiled `gitxtend` wheel vs the real `git` CLI.

The oracle is `git`: every assertion compares `gitxtend.<method>(...)` to the
output of the equivalent `git` command on the *same* temporary repository.
Standard library only (no pytest). Run after `maturin develop`:

    python -m unittest python.tests.test_e2e        # or
    python python/tests/test_e2e.py
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import unittest

import gitxtend

_ENV = {
    **os.environ,
    "GIT_CONFIG_GLOBAL": "/dev/null",
    "GIT_CONFIG_SYSTEM": "/dev/null",
    "GIT_AUTHOR_NAME": "qa",
    "GIT_AUTHOR_EMAIL": "qa@example.com",
    "GIT_COMMITTER_NAME": "qa",
    "GIT_COMMITTER_EMAIL": "qa@example.com",
}


def git(repo: str, *args: str) -> str:
    """Run `git -C repo <args>`, assert success, return trimmed stdout."""
    out = subprocess.run(
        ["git", "-C", repo, *args], env=_ENV, capture_output=True, text=True
    )
    if out.returncode != 0:
        raise AssertionError(f"git {args} failed: {out.stderr}")
    return out.stdout.strip()


def norm_iso(s: str) -> str:
    """git renders a UTC offset as `Z` (newer git) or `+00:00` (older); gitxtend
    always emits `+00:00`. Normalize for comparison."""
    return s[:-1] + "+00:00" if s.endswith("Z") else s


class GitxtendE2E(unittest.TestCase):
    def mkrepo(self) -> str:
        """Fresh repo on `main` with one commit. Auto-cleaned."""
        d = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, d, ignore_errors=True)
        git(d, "init", "-q", "-b", "main")
        with open(os.path.join(d, "README"), "w") as fh:
            fh.write("init\n")
        git(d, "add", "-A")
        git(d, "commit", "-q", "-m", "init")
        return d

    def bare(self) -> str:
        d = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, d, ignore_errors=True)
        git(d, "init", "--bare", "-q", "-b", "main")
        return d

    def commit(self, repo: str, name: str, msg: str) -> None:
        with open(os.path.join(repo, name), "w") as fh:
            fh.write(msg + "\n")
        git(repo, "add", "-A")
        git(repo, "commit", "-q", "-m", msg)

    def with_remote(self) -> tuple[str, str]:
        """(repo, bare) with `origin/main` pushed and in sync."""
        r = self.mkrepo()
        b = self.bare()
        git(r, "remote", "add", "origin", b)
        git(r, "push", "-q", "-u", "origin", "main")
        return r, b

    def advance_remote(self, bare: str) -> None:
        c = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, c, ignore_errors=True)
        git(c, "clone", "-q", bare, ".")
        self.commit(c, "r.txt", "remote")
        git(c, "push", "-q", "origin", "main")

    # ---- read primitives -------------------------------------------------

    def test_is_git_repo(self):
        r = self.mkrepo()
        self.assertTrue(gitxtend.is_git_repo(r))
        sub = os.path.join(r, "sub")
        os.makedirs(sub)
        self.assertTrue(gitxtend.is_git_repo(sub))
        nonrepo = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, nonrepo, ignore_errors=True)
        self.assertFalse(gitxtend.is_git_repo(nonrepo))

    def test_head_sha(self):
        r = self.mkrepo()
        self.assertEqual(gitxtend.head_sha(r), git(r, "rev-parse", "HEAD"))
        self.commit(r, "a.txt", "two")
        self.assertEqual(gitxtend.head_sha(r), git(r, "rev-parse", "HEAD"))

    def test_current_branch(self):
        r = self.mkrepo()
        self.assertEqual(gitxtend.current_branch(r), "main")
        git(r, "checkout", "--detach", git(r, "rev-parse", "HEAD"))
        self.assertIsNone(gitxtend.current_branch(r))

    def test_tracking_branch(self):
        r = self.mkrepo()
        self.assertIsNone(gitxtend.tracking_branch(r))
        r2, _ = self.with_remote()
        self.assertEqual(gitxtend.tracking_branch(r2), "origin/main")

    def test_remote_head_sha(self):
        r = self.mkrepo()
        self.assertIsNone(gitxtend.remote_head_sha(r, "origin/main"))
        r2, _ = self.with_remote()
        self.assertEqual(
            gitxtend.remote_head_sha(r2, "origin/main"),
            git(r2, "rev-parse", "origin/main"),
        )

    def test_ahead_behind(self):
        r, b = self.with_remote()
        self.assertEqual(gitxtend.ahead_behind(r, "origin/main"), (0, 0))
        self.commit(r, "x.txt", "local1")
        self.commit(r, "y.txt", "local2")
        self.assertEqual(gitxtend.ahead_behind(r, "origin/main"), (2, 0))
        self.advance_remote(b)
        git(r, "fetch", "-q")
        self.assertEqual(gitxtend.ahead_behind(r, "origin/main"), (2, 1))

    def test_rev_list_count(self):
        r = self.mkrepo()
        self.commit(r, "a.txt", "two")
        self.assertEqual(
            gitxtend.rev_list_count(r, "HEAD"),
            int(git(r, "rev-list", "--count", "HEAD")),
        )
        self.assertEqual(gitxtend.rev_list_count(r, "nope..HEAD"), 0)

    def test_log_subjects(self):
        r = self.mkrepo()
        self.commit(r, "a.txt", "two")
        self.commit(r, "b.txt", "three")
        self.assertEqual(gitxtend.log_subjects(r, "HEAD", 2), ["three", "two"])
        self.assertEqual(
            gitxtend.log_subjects(r, "HEAD", 10),
            git(r, "log", "--format=%s", "--max-count=10", "HEAD").splitlines(),
        )

    def test_is_clean(self):
        r = self.mkrepo()
        self.assertTrue(gitxtend.is_clean(r))
        with open(os.path.join(r, "untracked.txt"), "w") as fh:
            fh.write("x")
        self.assertFalse(gitxtend.is_clean(r))

    def test_status_counts(self):
        r = self.mkrepo()
        self.assertEqual(gitxtend.status_counts(r), (0, 0))
        with open(os.path.join(r, "u.txt"), "w") as fh:
            fh.write("x")
        self.assertEqual(gitxtend.status_counts(r), (0, 1))

    def test_remote_urls(self):
        r = self.mkrepo()
        self.assertEqual(gitxtend.remote_urls(r), {})
        git(r, "remote", "add", "origin", "https://example.com/x.git")
        self.assertEqual(gitxtend.remote_urls(r), {"origin": "https://example.com/x.git"})

    def test_last_commit_date(self):
        r = self.mkrepo()
        self.assertEqual(
            gitxtend.last_commit_date(r), norm_iso(git(r, "log", "-1", "--format=%aI"))
        )

    def test_fetch(self):
        r, b = self.with_remote()
        self.advance_remote(b)
        self.assertTrue(gitxtend.fetch(r, None))
        self.assertEqual(
            git(r, "rev-parse", "origin/main"),
            gitxtend.remote_head_sha(r, "origin/main"),
        )
        self.assertFalse(gitxtend.fetch(r, "does-not-exist"))

    # ---- roll-up ---------------------------------------------------------

    def test_repo_status_error(self):
        self.assertEqual(
            gitxtend.repo_status("/definitely/not/real/xyzzy", False).sync_state, "error"
        )
        nonrepo = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, nonrepo, ignore_errors=True)
        self.assertEqual(gitxtend.repo_status(nonrepo, False).sync_state, "error")

    def test_repo_status_no_remote(self):
        s = gitxtend.repo_status(self.mkrepo(), False)
        self.assertEqual(s.sync_state, "no-remote")
        self.assertIsNone(s.tracking_branch)

    def test_repo_status_up_to_date(self):
        r, _ = self.with_remote()
        s = gitxtend.repo_status(r, False)
        self.assertEqual(s.sync_state, "up-to-date")
        self.assertEqual(s.tracking_branch, "origin/main")
        self.assertEqual((s.ahead_count, s.behind_count), (0, 0))

    def test_repo_status_ahead(self):
        r, _ = self.with_remote()
        self.commit(r, "x.txt", "local")
        s = gitxtend.repo_status(r, False)
        self.assertEqual(s.sync_state, "ahead")
        self.assertEqual((s.ahead_count, s.behind_count), (1, 0))

    def test_repo_status_diverged(self):
        r, b = self.with_remote()
        self.commit(r, "l.txt", "local")
        self.advance_remote(b)
        s = gitxtend.repo_status(r, True)  # fetch=True
        self.assertEqual(s.sync_state, "diverged")
        self.assertEqual((s.ahead_count, s.behind_count), (1, 1))


if __name__ == "__main__":
    unittest.main()
