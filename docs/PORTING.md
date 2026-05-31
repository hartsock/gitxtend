# gitxtend — Porting Guide (git CLI → gix)

Per-method mapping from git-tend's `GitService` (which shells out to `git`) to
gitoxide (`gix`) calls. This is the implementation checklist for `src/repo.rs`
and `src/status.rs`. Crate versions are not pinned here — pin `gix` to a recent
release in `Cargo.toml` and use a recent stable Rust toolchain.

Legend: **CLI** = what git-tend runs today · **gix** = intended approach.

---

### is_git_repo(path) -> bool
- **CLI:** `git rev-parse --git-dir` (exit 0)
- **gix:** `gix::open(path).is_ok()` (or `gix::discover` if you want to honor
  the "inside a repo, not just at the root" semantics — `rev-parse --git-dir`
  succeeds from subdirectories, so prefer `gix::discover`).

### is_clean(path) -> bool
- **CLI:** `git status --porcelain` is empty
- **gix:** open repo, run a status that includes worktree modifications and
  untracked files; clean == no entries. (gix `status` platform; ensure
  untracked + ignored handling matches porcelain defaults.)

### current_branch(path) -> str | None
- **CLI:** `git rev-parse --abbrev-ref HEAD`; `None` when output == `HEAD`
- **gix:** `repo.head()?`; if detached return `None`, else the short ref name.

### tracking_branch(path) -> str | None
- **CLI:** `git rev-parse --abbrev-ref @{upstream}`
- **gix:** from the current branch ref, resolve its configured upstream
  (`branch.<name>.remote` + `.merge`) to a short `remote/branch` name.

### fetch(path, remote=None) -> bool
- **CLI:** `git fetch <remote>` or `git fetch --all`
- **gix:** gix supports fetch, but network fetch is the least-mature path in
  scope. **Decision:** implement behind the `fetch()` signature with a
  preference order — (1) try `gix` fetch; if the build/feature proves
  unreliable, (2) fall back to shelling out to `git fetch` *inside the Rust
  module* (still one process from Python's view). Document which path shipped.
  Honor credentials/SSH exactly as the user's git does (respect `~/.gitconfig`,
  ssh-agent). This is the single riskiest method — keep it isolated.

### head_sha(path) -> str | None
- **CLI:** `git rev-parse HEAD`
- **gix:** `repo.head_id()?` → hex string; `None` on unborn/empty repo.

### remote_head_sha(path, remote_ref="origin/main") -> str | None
- **CLI:** `git rev-parse origin/main` (after fetch)
- **gix:** resolve the remote-tracking ref (`refs/remotes/<remote_ref>`) to its
  object id. `None` if the ref doesn't exist.

### ahead_behind(path, upstream) -> (int, int)
- **CLI (today, two calls):**
  `git rev-list --count {upstream}..HEAD` (ahead) and
  `git rev-list --count HEAD..{upstream}` (behind)
- **gix:** single merge-base + graph walk to count commits unique to each side.
  Return `(ahead, behind)`. This is the headline efficiency win — one walk
  instead of two `git` forks.

### rev_list_count(path, range_spec) -> int  *(kept for compatibility)*
- **CLI:** `git rev-list --count <range_spec>`
- **gix:** parse a two-dot `A..B` range into endpoints and count via the same
  walker used by `ahead_behind`. Soft-fail to `0` on parse/lookup error to
  match current behaviour.

### log_subjects(path, range_spec, max_count=10) -> list[str]
- **CLI:** `git log --format=%s --max-count=N <range_spec>`
- **gix:** walk the range newest-first, take `max_count`, return each commit's
  summary line (first line of the message). Soft-fail to `[]`.

### remote_urls(path) -> dict[str, str]
- **CLI:** parse `git remote -v` `(fetch)` lines
- **gix:** read remotes from config; map each remote name to its fetch URL.
  Soft-fail to `{}`.

### last_commit_date(path) -> str | None
- **CLI:** `git log -1 --format=%aI` (author date, ISO 8601 strict)
- **gix:** HEAD commit's author time, formatted as RFC3339/ISO-8601 with offset
  to match `%aI` exactly. `None` on empty repo.

### status_counts(path) -> (int, int)
- **CLI:** parse `git status --porcelain`: lines starting `??` are untracked,
  all other non-empty lines are modified
- **gix:** from the same status used by `is_clean`, bucket entries into
  (modified, untracked) with the **same definition** porcelain uses (an
  untracked file is `??`; everything else — staged or unstaged change, rename,
  delete — counts as modified). Verify against fixtures.

---

## repo_status(path, fetch) -> RepoStatus  (src/status.rs)

Port the source `check_repo` **verbatim**. Order and error strings matter —
tests assert on them. The struct field is `sync_state` (not `state`); `DIRTY`
is **never** returned here.

```
path = expanduser(resolve(path))
if not path.exists():       -> sync_state=ERROR, error=f"Directory not found: {path}"; return
if not is_git_repo(path):   -> sync_state=ERROR, error=f"Not a git repository: {path}"; return

local_branch = current_branch(path)
tracking     = tracking_branch(path)

if tracking is None:        -> sync_state=NO_REMOTE,
                               local_branch, local_sha=head_sha(path),
                               is_dirty=not is_clean(path);
                               (tracking_branch left None); return

if fetch:
    ok, stderr = _fetch(path)
    if not ok:              -> sync_state=ERROR, local_branch, tracking_branch=tracking,
                               error=f"Fetch failed: {stderr}"; return

local_sha  = head_sha(path)
remote_sha = remote_head_sha(path, tracking)
is_dirty   = not is_clean(path)

if local_sha == remote_sha:
    sync_state = UP_TO_DATE
else:
    ahead  = rev_list_count(f"{tracking}..HEAD")
    behind = rev_list_count(f"HEAD..{tracking}")
    sync_state = DIVERGED if (ahead>0 and behind>0) else BEHIND if behind>0 else AHEAD

ahead_count  = rev_list_count(f"{tracking}..HEAD")      # always recomputed
behind_count = rev_list_count(f"HEAD..{tracking}")
new_remote_commits = log_subjects(f"HEAD..{tracking}", 10) if behind_count>0 else []
return RepoStatus(path, sync_state, local_branch, tracking, local_sha, remote_sha,
                  ahead_count, behind_count, new_remote_commits, is_dirty)
```

Notes for the implementor:
- `check_repo` **never** yields `DIRTY`; `is_dirty` is a flag only. The `DIRTY`
  state lives in the separate *scan* path (workspace auto-discovery) — port that
  alongside, not inside, `repo_status`.
- A fetch **failure** becomes `sync_state=ERROR` with `error="Fetch failed:
  {stderr}"`, so the roll-up needs fetch's stderr, not just a bool. Provide an
  internal `_fetch(path) -> (ok, stderr)`; the public `fetch()` may still return
  `bool` for the per-method shim.
- `ahead_behind()` is the efficiency win, but to stay byte-compatible the
  roll-up may call it once and reuse the pair for both the decision and the
  counts.
- Keep every `error` string identical to the text above.

---

## Testing strategy

- **Rust unit tests** in `repo.rs`/`status.rs` against temp-dir fixtures built
  with `gix` (init repo, make commits, set upstream, dirty the tree, diverge).
  Cover every state in the SyncState tree.
- **Parity tests:** for each method, assert the gix result equals the result of
  the real `git` CLI on the same fixture. This is the acceptance bar — gitxtend
  must agree with `git` on every fixture before the plugin adopts it.
- **Python smoke tests** post-`maturin develop`: import the module, run
  `repo_status()` on a fixture, assert fields.

Rule of thumb: every behaviour needs a regression test; mock/contain
external resources.
