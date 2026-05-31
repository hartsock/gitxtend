# gitxtend — Design

## Goal

Replace git-tend's subprocess-based git layer with an in-process,
gitoxide-backed implementation delivered as a single PyO3 extension module.
Behaviour-compatible with the existing `GitService` so adoption is a one-line
import swap in the Python plugin.

## Background: the seam we are replacing

In `gila-plugin-git-tend`, **all** git operations funnel through one class:

```python
# gila_plugin_git_tend/services/git_service.py
class GitService:
    def run(self, path, args):
        return subprocess.run(["git"] + args, cwd=path,
                              capture_output=True, text=True, timeout=120)
    # is_git_repo, is_clean, current_branch, tracking_branch, fetch, pull,
    # push, add, commit, stash_push, stash_pop, create_branch, reset_hard,
    # head_sha, remote_head_sha, rev_list_count, log_oneline, remote_urls,
    # last_commit_date, status_counts
```

Everything above `GitService` (`StatusService`, `ScanService`, `TendService`,
`PRService`, config, board, CLI) is untouched by this project. They consume
`GitService` purely through its method surface. That surface is our contract.

## Why gitoxide

- **In-process.** `gix` opens a repo once and answers many queries without
  forking. A workspace scan over N repos goes from O(N × several `git` forks)
  to a single process walking each repo's object DB.
- **Deterministic.** No dependence on the ambient `git` version, aliases, or
  config injected via `PATH`. The git semantics are compiled in and pinned.
- **Pure-Rust core.** Read operations (status, ahead/behind, rev-list, log,
  refs, remotes) are well covered by `gix` without linking libgit2 or shelling
  out.

### Where gitoxide is still maturing

`gix`'s **read** path is solid; its **network** (fetch/push) and some
**mutation** paths are less mature than libgit2. This is the central reason the
roadmap ports the read side first and leaves fetch/push/commit/stash in the
Python plugin initially. `fetch` is the one network call in v1 read-side scope
and gets special handling (see PORTING.md → `fetch`).

## Artifact shape

**Primary:** a PyO3 extension module (`cdylib`) built by maturin into a Python
wheel. The plugin does `from gitxtend import repo_status, ...`.

**Optional (later):** a `[[bin]]` target in the same crate producing a
standalone `gitxtend` CLI sharing the same core `repo.rs`/`status.rs`, for use
outside Python (cron, shell). Not required for v1; the crate is structured so
this is additive.

## Module structure

```
src/lib.rs      #[pymodule] — registers the Python-visible functions/classes,
                converts gix errors → Python exceptions, maps Rust types →
                Python (str/int/bool/list/dataclass-like PyRepoStatus).

src/repo.rs     The gix-backed primitives, one per GitService read method.
                Pure Rust, no PyO3 — unit-testable with gix fixtures.

src/status.rs   repo_status(): the StatusService.check_repo roll-up, including
                the SyncState decision tree, expressed in Rust over repo.rs.
```

Keeping `repo.rs`/`status.rs` PyO3-free means they can be unit-tested in Rust
and reused by an optional CLI `bin` target.

## Error & type mapping

- gix errors → raise `RuntimeError` (or a dedicated `GitxtendError`) with the
  underlying message. Read methods that git-tend treats as "soft failures"
  (e.g. `rev_list_count` returns `0` on error, `head_sha` returns `None`)
  preserve that exact behaviour rather than raising, to stay drop-in.
- `Path` arguments accept `str` or `os.PathLike`.
- `RepoStatus` is exposed as a `#[pyclass]` with the same fields as the
  Python dataclass (see API.md), so existing formatting code keeps working.

## Non-goals

- Reimplementing the CLI, YAML config, forge (gh/glab) integration, the
  knowledge-board logic, or the systemd timer. Those stay in Python.
- Replacing the write/merge/conflict machinery in v1.
- Being a general-purpose git library. Scope is exactly git-tend's needs.
