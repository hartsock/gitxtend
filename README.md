# gitxtend

A single, self-contained binary that drives Git repository *tending* —
detecting unpushed commits, untracked work, and out-of-sync branches across
many repositories — backed by [gitoxide (`gix`)][gix] and exposed to Python
through [PyO3]/[maturin].

> **Status: v0.1.0 — read side implemented, plus the submodule command.** All 13
> read primitives and the `repo_status` roll-up are implemented (Rust/gix) and
> exposed to Python, each with parity tests vs the `git` CLI and an end-to-end
> suite. On top of that: `gitxtend submodule sync`, the one-command
> [submodule updater](#keeping-a-repo-full-of-submodules-up-to-date), available
> as a standalone binary and as a Python console script. Next: plugin adoption
> and the rest of the write side — see [`docs/ROADMAP.md`](docs/ROADMAP.md).
> [`docs/DESIGN.md`](docs/DESIGN.md) and [`docs/PORTING.md`](docs/PORTING.md)
> cover the architecture.

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

## What it does (v0.1.0 — the read side)

The first milestone ports the **read side** of tending — the part that *detects*
work that needs attention, without mutating any repo. All of it is implemented:

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
| Submodule status | — | `submodule_status(path, recursive=True)` |
| Submodule update (+record) | — | `update_submodules(path, ..., commit=False)` · CLI: `gitxtend submodule sync` |

The **write side** (`pull --ff-only`, `push`, `add`, `commit`, `stash`,
`branch`, `reset --hard`) stays in the host tool shelling out to `git` until
the read path is proven in production. See [`docs/ROADMAP.md`](docs/ROADMAP.md).

**One deliberate exception:** the submodule commands below *do* mutate — they
check out submodules and, with `--commit`, write a commit in the superproject.
They are also the one place this crate does not use gix: submodule update and
status are delegated to the local `git` CLI on purpose, so the semantics are
Git's own rather than a reimplementation of them. Submodule updating is the
use case that motivated the CLI, and it is self-contained enough not to wait on
the general write-side port.

## Keeping a repo full of submodules up to date

One command moves every submodule to the tip of the branch it tracks:

```bash
gitxtend submodule sync                 # the current directory
gitxtend submodule sync ~/src/myrepo    # or a named one
```

```
modA  9bfc36b -> 3bb8896
modB  2211b7a -> 4ff51bd  (devel)

2 submodules advanced (not recorded — re-run with --commit)
```

`--commit` records the moves in the superproject, which is what makes them
stick:

```bash
gitxtend submodule sync --commit
gitxtend submodule sync --commit -m "bump vendored deps"
```

```
2 submodules advanced, recorded as f14b311
```

Also available: `gitxtend submodule status` for a structured view, `--json` on
either subcommand, `--no-remote` to *restore* submodules to the recorded SHAs
instead of advancing them, and `--no-recursive`. `gitxtend --help` has the rest.
Exit codes are `0` ok, `1` a git operation failed, `2` bad usage.

### Why `--commit` matters

`git submodule update --remote` moves each submodule's working tree and stops
there — leaving a detached HEAD in each submodule and a **modified gitlink** in
the superproject. By itself it makes the superproject *dirty*, not *up to date*:
the next plain `git submodule update` snaps everything back to the SHA the
superproject still records. Recording the bumps is a separate commit, and
`--commit` is it.

Two caveats worth knowing:

- With `--recursive`, a nested submodule shows up as `outer/inner`. The
  superproject cannot stage that path, so `--commit` records **top-level**
  gitlinks only; a nested bump needs a commit inside `outer` first.
- A submodule with no `branch =` line in `.gitmodules` follows the remote's
  default branch. Those print without a branch annotation, rather than being
  guessed at.

### Two front ends, one program

The command ships twice: as the standalone `gitxtend` binary (no Python needed —
good for cron) and as a console script in the wheel. Both are thin shims over
the same `cli::run` in the library, so they cannot disagree about a flag, an
output line, or an exit code — a parity test runs both over the same argv and
compares. `python -m gitxtend` works too.

```bash
cargo build --release --bin gitxtend    # the standalone binary
pip install gitxtend                    # puts the console script on PATH
```

If both are installed, whichever comes first on `PATH` wins; they behave
identically.

### From Python

```python
import gitxtend

report = gitxtend.update_submodules("~/src/myrepo", commit=True)
for change in report.changed:
    print(change.path, change.from_commit, "->", change.to_commit, change.branch)
print(report.commit)   # superproject commit that recorded the bumps, or None
```

Idempotent: a repeat run reports nothing changed and makes no empty commit.

## Layout

```
gitxtend/
├── Cargo.toml            # Rust crate (cdylib for PyO3 + the `gitxtend` bin)
├── pyproject.toml        # maturin build backend → Python wheel + console script
├── src/
│   ├── lib.rs            # crate root (cli/error/repo/status; python feature)
│   ├── cli.rs            # the `gitxtend` command: argv → (code, stdout, stderr)
│   ├── main.rs           # the standalone binary — a shim over cli::run
│   ├── python.rs         # PyO3 module entry — #[pymodule] gitxtend (feature-gated)
│   ├── repo/             # gix-backed read primitives, one file per method
│   └── status.rs         # repo_status roll-up + SyncState decision tree
├── python/gitxtend/
│   ├── _cli.py           # console script — the other shim over cli::run
│   ├── __main__.py       # `python -m gitxtend`
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
