"""End-to-end tests for the compiled `gitxtend` wheel vs the real `git` CLI.

The oracle is `git`: every assertion compares `gitxtend.<method>(...)` to the
output of the equivalent `git` command on the *same* temporary repository.
Standard library only (no pytest). Run after `maturin develop`:

    python -m unittest python.tests.test_e2e        # or
    python python/tests/test_e2e.py
"""

from __future__ import annotations

import contextlib
import io
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

import gitxtend
import pytest
from gitxtend import _cli

pytestmark = pytest.mark.integration

_REPO_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# Variables by which an ambient git process points its children at *its*
# repository, overriding `-C`. A pre-push hook runs with GIT_DIR set, so
# inheriting these would silently retarget every fixture command at the real
# checkout. Mirrors `AMBIENT_REPO_ENV` in src/repo/mod.rs.
_AMBIENT_REPO_ENV = (
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
)

_ENV = {
    **{k: v for k, v in os.environ.items() if k not in _AMBIENT_REPO_ENV},
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


def git_allow_file_protocol(repo: str, *args: str) -> str:
    out = subprocess.run(
        ["git", "-C", repo, "-c", "protocol.file.allow=always", *args],
        env=_ENV,
        capture_output=True,
        text=True,
    )
    if out.returncode != 0:
        raise AssertionError(f"git {args} failed: {out.stderr}")
    return out.stdout.strip()


def run_cli(*args: str) -> tuple[int, str, str]:
    """Drive the console script in-process: (exit_code, stdout, stderr).

    Deliberately goes through `gitxtend._cli.main` — the real console-script
    entry point — rather than the compiled `cli_main` underneath it, so the
    Python shim's stream forwarding and exit code are exercised too. Calling the
    Rust function directly would leave the only untested code in the package.
    """
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = _cli.main(list(args))
    return code, out.getvalue(), err.getvalue()


def rust_binary() -> str:
    """Path to the standalone `gitxtend` binary, for the front-end parity test.

    Skips locally when the binary has not been built, but NEVER on CI — a
    silently-skipped parity test is a green that proves nothing, and CI is the
    authoritative gate.
    """
    override = os.environ.get("GITXTEND_BIN")
    candidates = [override] if override else [
        os.path.join(_REPO_ROOT, "target", profile, "gitxtend")
        for profile in ("release", "debug")
    ]
    for candidate in candidates:
        if candidate and os.path.isfile(candidate) and os.access(candidate, os.X_OK):
            return candidate
    message = (
        "the standalone gitxtend binary is not built "
        f"(looked in {candidates}); run `cargo build --bin gitxtend`"
    )
    if os.environ.get("CI"):
        raise AssertionError(message)
    raise unittest.SkipTest(message)


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

    def mksuper(self, branch: str | None = None) -> tuple[str, str]:
        """A superproject with one submodule at `mods`, returned as (parent, child).

        `branch` is the branch the submodule tracks. The file transport is
        enabled in both the parent and the submodule config because a `--remote`
        update fetches from *inside* the submodule, which reads its own config.
        """
        parent = self.mkrepo()
        child = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, child, ignore_errors=True)
        git(child, "init", "-q", "-b", "main")
        self.commit(child, "hello.txt", "init child")
        if branch:
            git(child, "checkout", "-q", "-b", branch)
            self.commit(child, "hello.txt", "child on branch")

        git(parent, "config", "protocol.file.allow", "always")
        git(parent, "config", "user.name", "qa")
        git(parent, "config", "user.email", "qa@example.com")

        add = ["submodule", "add"]
        if branch:
            add += ["-b", branch]
        git_allow_file_protocol(parent, *add, child, "mods")
        git(parent, "add", "-A")
        git(parent, "commit", "-q", "-m", "add submodule")
        git(os.path.join(parent, "mods"), "config", "protocol.file.allow", "always")
        return parent, child

    def advance(self, child: str, branch: str | None = None) -> str:
        if branch:
            git(child, "checkout", "-q", branch)
        self.commit(child, "hello.txt", "advance")
        return git(child, "rev-parse", "HEAD")

    def test_fixture_env_scrubs_the_ambient_repo_pointers(self):
        """Regression: the E2E fixture env must not inherit GIT_DIR & friends.

        Under a pre-push hook `GIT_DIR` is set, and it overrides `git -C`. An
        inherited one pointed the fixtures at the developer's own checkout.
        """
        for key in _AMBIENT_REPO_ENV:
            self.assertNotIn(key, _ENV)

    def test_submodule_status(self):
        parent, _child = self.mksuper()

        status = gitxtend.submodule_status(parent)
        self.assertEqual(status[0][0], "mods")
        self.assertEqual(status[0][1], "clean")

        shutil.rmtree(os.path.join(parent, "mods"))
        self.assertEqual(gitxtend.submodule_status(parent)[0][1], "not-initialized")
        ok, stderr = gitxtend.sync_submodules(parent, True, False)
        self.assertTrue(ok, stderr)
        self.assertEqual(gitxtend.submodule_status(parent)[0][1], "clean")

    def test_update_submodules_advances_and_records(self):
        parent, child = self.mksuper("devel")
        tip = self.advance(child, "devel")

        report = gitxtend.update_submodules(parent, commit=True)

        self.assertTrue(report.ok, report.stderr)
        self.assertEqual(len(report.changed), 1)
        change = report.changed[0]
        self.assertEqual(change.path, "mods")
        self.assertEqual(change.to_commit, tip)
        self.assertEqual(change.branch, "devel")
        self.assertFalse(change.initialized)
        # Recording the bump is what leaves the superproject clean.
        self.assertIsNotNone(report.commit)
        self.assertEqual(git(parent, "status", "--porcelain"), "")
        self.assertEqual(git(parent, "rev-parse", "HEAD:mods"), tip)

    def test_update_submodules_is_idempotent(self):
        parent, child = self.mksuper("devel")
        self.advance(child, "devel")
        first = gitxtend.update_submodules(parent, commit=True)
        self.assertTrue(first.ok, first.stderr)
        head = git(parent, "rev-parse", "HEAD")

        second = gitxtend.update_submodules(parent, commit=True)

        self.assertTrue(second.ok, second.stderr)
        self.assertEqual(second.changed, [])
        self.assertIsNone(second.commit)
        self.assertEqual(git(parent, "rev-parse", "HEAD"), head)

    def test_cli_sync_updates_submodules_in_one_command(self):
        parent, child = self.mksuper("devel")
        tip = self.advance(child, "devel")

        code, out, err = run_cli("submodule", "sync", parent, "--commit")

        self.assertEqual(code, 0, err)
        self.assertIn("mods", out)
        self.assertIn("(devel)", out)
        self.assertIn("recorded as", out)
        self.assertEqual(git(parent, "rev-parse", "HEAD:mods"), tip)
        self.assertEqual(git(parent, "status", "--porcelain"), "")

    def test_cli_sync_without_commit_leaves_the_bump_uncommitted(self):
        parent, child = self.mksuper("devel")
        self.advance(child, "devel")

        code, out, _err = run_cli("submodule", "sync", parent)

        self.assertEqual(code, 0)
        self.assertIn("--commit", out, "must point at the flag that records it")
        self.assertNotEqual(git(parent, "status", "--porcelain"), "")

    def test_python_dash_m_runs_the_same_command(self):
        """`python -m gitxtend` is a supported entry point, so pin it.

        Out of process by necessity (the module raises SystemExit at import),
        which is also why `__main__.py` carries a no-cover pragma.
        """
        proc = subprocess.run(
            [sys.executable, "-m", "gitxtend", "--version"],
            env=_ENV,
            capture_output=True,
            text=True,
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stdout, run_cli("--version")[1])

    def test_cli_usage_error_exits_2(self):
        code, _out, err = run_cli("submodule", "definitely-not-a-subcommand")
        self.assertEqual(code, 2)
        self.assertIn("USAGE", err)

    def test_cli_front_ends_are_the_same_program(self):
        """The console script and the standalone binary must agree exactly.

        Both are shims over `gitxtend::cli::run`, so this asserts that claim
        rather than trusting it: same argv, byte-identical streams and code.
        """
        binary = rust_binary()
        parent, child = self.mksuper("devel")
        self.advance(child, "devel")

        for argv in (
            ["submodule", "status", parent],
            ["submodule", "status", parent, "--json"],
            ["--version"],
            ["submodule", "bogus"],
        ):
            with self.subTest(argv=argv):
                via_python = run_cli(*argv)
                proc = subprocess.run(
                    [binary, *argv], env=_ENV, capture_output=True, text=True
                )
                self.assertEqual(
                    via_python, (proc.returncode, proc.stdout, proc.stderr)
                )

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
