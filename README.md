# gitxtend

A single, self-contained binary that drives Git repository *tending* —
detecting unpushed commits, untracked work, and out-of-sync branches across
many repositories — backed by [gitoxide (`gix`)][gix] and exposed to Python
through [PyO3]/[maturin].

> **Status: scaffold / specification.** This repository currently contains the
> design, the API contract, and build stubs. See
> [`docs/DESIGN.md`](docs/DESIGN.md) and [`docs/PORTING.md`](docs/PORTING.md)
> for exactly what to implement.

## Why this exists

A Python repository-*tending* tool (`git-tend`) already does this well, but
every git operation forks the `git` CLI via `subprocess.run(["git", ...])`.
A `status` / `scan` across a workspace of N repos spawns dozens of short-lived
`git` processes per run, and the tool's behaviour is coupled to whatever `git`
binary and version happens to be on `PATH`.

`gitxtend` replaces that seam with **in-process git** via gitoxide:

- **No fork-per-call.** A scan of a whole workspace runs in one process.
- **No `git` on `PATH` dependency.** The git logic is compiled in.
- **One artifact.** A single compiled module (`.so` wheel) — or, optionally, a
  standalone CLI binary — carries the whole git layer.
- **Same contract.** It re-implements the exact method surface of the Python
  `GitService` git layer it replaces, so the tending tool can adopt it with a
  one-line import swap.

The motivating incident: a local-only **unpushed** commit on `main` was nearly
lost during a merge+reset. Tending is the discipline that catches that;
`gitxtend` makes tending fast enough to run constantly.

## What it will do (v1 scope)

The first milestone ports the **read side** of tending — the part that *detects*
work that needs attention, without mutating any repo:

| Capability | git-tend method(s) | gitxtend |
|---|---|---|
| Is this a git repo? | `is_git_repo` | `is_git_repo(path)` |
| Working tree clean? | `is_clean` | `is_clean(path)` |
| Current / tracking branch | `current_branch`, `tracking_branch` | `current_branch`, `tracking_branch` |
| HEAD & remote SHAs | `head_sha`, `remote_head_sha` | `head_sha`, `remote_head_sha` |
| Ahead / behind counts | `rev_list_count` | `ahead_behind(path, upstream)` |
| New remote commit subjects | `log_oneline` | `log_subjects(path, range, max)` |
| Remote names → URLs | `remote_urls` | `remote_urls(path)` |
| Last commit date (ISO 8601) | `last_commit_date` | `last_commit_date(path)` |
| Modified / untracked counts | `status_counts` | `status_counts(path)` |
| Fetch from remote | `fetch` | `fetch(path, remote=None)` |
| **Roll-up** | `check_repo` | `repo_status(path, fetch=True) -> RepoStatus` |

The **write side** (`pull --ff-only`, `push`, `add`, `commit`, `stash`,
`branch`, `reset --hard`) stays in the host tool shelling out to `git` until
the read path is proven in production. See [`docs/ROADMAP.md`](docs/ROADMAP.md).

## Layout

```
gitxtend/
├── Cargo.toml            # Rust crate (cdylib for PyO3; optional bin target)
├── pyproject.toml        # maturin build backend → Python wheel
├── src/
│   ├── lib.rs            # PyO3 module entry — #[pymodule] gitxtend
│   ├── repo.rs           # gix-backed read operations (TODO)
│   └── status.rs         # RepoStatus roll-up + SyncState logic (TODO)
├── python/gitxtend/
│   └── __init__.pyi      # type stubs for the compiled module
└── docs/
    ├── DESIGN.md         # architecture & rationale
    ├── API.md            # the exact Python-visible surface to implement
    ├── PORTING.md        # git CLI command → gix mapping, per method
    └── ROADMAP.md        # milestones; read-side first, write-side later
```

## Building

```bash
# from a checkout, inside your Python virtualenv
maturin develop --release      # build + install into the active venv
# or, to produce a distributable wheel:
maturin build --release
```

Toolchain: a recent stable Rust, `maturin`, Python 3.11+.

## Integration target

Drop-in for a Python `GitService` git layer's read methods. The host tool keeps
its CLI, config, forge (gh/glab), and board logic; only the git layer changes.
See [`docs/API.md`](docs/API.md) for the adapter shape.

## License

Apache License 2.0 — see [`LICENSE`](LICENSE) and [`NOTICE`](NOTICE).

[gix]: https://github.com/Byron/gitoxide
[PyO3]: https://pyo3.rs
[maturin]: https://www.maturin.rs
