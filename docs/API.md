# gitxtend — Python API Contract

This is the exact Python-visible surface the compiled module must expose. It
mirrors `gila_plugin_git_tend.services.git_service.GitService` (read side) plus
one roll-up that mirrors `StatusService.check_repo`.

Type stubs live in [`../python/gitxtend/__init__.pyi`](../python/gitxtend/__init__.pyi).

## Conventions

- `path` accepts `str | os.PathLike`. Internally resolved to an absolute path.
- "Soft-fail" methods mirror git-tend's current behaviour exactly: they return
  a sentinel (`None`, `0`, `[]`, `{}`) instead of raising, so callers don't
  change. These are marked **soft-fail** below.
- Methods that git-tend lets propagate process errors may raise
  `GitxtendError` (subclass of `RuntimeError`).

## Read primitives (port of GitService)

```python
def is_git_repo(path) -> bool
    # GitService.is_git_repo — `git rev-parse --git-dir` exit==0

def is_clean(path) -> bool
    # GitService.is_clean — `git status --porcelain` empty

def current_branch(path) -> str | None        # soft-fail (None if detached)
    # GitService.current_branch — `rev-parse --abbrev-ref HEAD`, None if "HEAD"

def tracking_branch(path) -> str | None        # soft-fail
    # GitService.tracking_branch — `rev-parse --abbrev-ref @{upstream}`

def head_sha(path) -> str | None               # soft-fail
    # GitService.head_sha — `rev-parse HEAD`

def remote_head_sha(path, remote_ref="origin/main") -> str | None   # soft-fail
    # GitService.remote_head_sha — `rev-parse <remote_ref>` (after fetch)

def ahead_behind(path, upstream) -> tuple[int, int]
    # Replaces two GitService.rev_list_count calls:
    #   ahead  = rev_list_count(f"{upstream}..HEAD")
    #   behind = rev_list_count(f"HEAD..{upstream}")
    # gix computes both in one graph walk. Returns (ahead, behind).

def rev_list_count(path, range_spec) -> int    # soft-fail (0 on error)
    # GitService.rev_list_count — kept for 1:1 compatibility / other callers

def log_subjects(path, range_spec, max_count=10) -> list[str]   # soft-fail
    # GitService.log_oneline — commit subjects (%s) in range, newest first

def remote_urls(path) -> dict[str, str]        # soft-fail ({} on error)
    # GitService.remote_urls — {remote_name: fetch_url}

def last_commit_date(path) -> str | None       # soft-fail
    # GitService.last_commit_date — ISO 8601 (%aI) of HEAD commit

def status_counts(path) -> tuple[int, int]     # soft-fail ((0,0) on error)
    # GitService.status_counts — (modified, untracked) from porcelain status
```

## The one network call in v1 scope

```python
def fetch(path, remote=None) -> bool
    # GitService.fetch — fetch <remote> or --all. Returns True on success.
    # See PORTING.md → fetch for the gix-vs-shell decision. May be a thin
    # shell-out behind the same signature if gix fetch proves unstable; the
    # Python caller must not care which.
```

## Roll-up (port of StatusService.check_repo)

```python
class RepoStatus:
    path: str
    state: str            # one of SyncState values (see below)
    local_branch: str | None
    tracking_branch: str | None
    local_sha: str | None
    remote_sha: str | None
    ahead_count: int
    behind_count: int
    new_remote_commits: list[str]
    is_dirty: bool
    error: str | None

def repo_status(path, fetch=True) -> RepoStatus
    # Mirrors StatusService.check_repo exactly:
    #   1. not a repo            -> state="error", error set
    #   2. no upstream           -> state="no-remote", is_dirty filled
    #   3. fetch (if requested)
    #   4. compute ahead/behind, fill new_remote_commits when behind>0
    #   5. decide state via the tree below
```

### SyncState values (exact strings, from models.SyncState)

`"up-to-date" | "ahead" | "behind" | "diverged" | "dirty" | "no-remote" | "error"`

### State decision tree (must match status_service.py)

```
ahead>0 and behind>0   -> "diverged"
ahead>0                -> "ahead"
behind>0               -> "behind"
is_dirty               -> "dirty"
else                   -> "up-to-date"
```

## Adapter for the plugin

git-tend can adopt this with a shim that keeps the old class name:

```python
# gila_plugin_git_tend/services/git_service.py  (read side)
import gitxtend

class GitService:
    def is_git_repo(self, path):       return gitxtend.is_git_repo(path)
    def is_clean(self, path):          return gitxtend.is_clean(path)
    def current_branch(self, path):    return gitxtend.current_branch(path)
    def tracking_branch(self, path):   return gitxtend.tracking_branch(path)
    def head_sha(self, path):          return gitxtend.head_sha(path)
    def remote_head_sha(self, p, r="origin/main"):
                                       return gitxtend.remote_head_sha(p, r)
    def rev_list_count(self, p, spec): return gitxtend.rev_list_count(p, spec)
    def log_oneline(self, p, spec, max_count=10):
                                       return gitxtend.log_subjects(p, spec, max_count)
    def remote_urls(self, path):       return gitxtend.remote_urls(path)
    def last_commit_date(self, path):  return gitxtend.last_commit_date(path)
    def status_counts(self, path):     return gitxtend.status_counts(path)
    def fetch(self, path, remote=None):return gitxtend.fetch(path, remote)
    # write methods (pull/push/add/commit/stash/branch/reset) unchanged for now
```

Or, better, route `StatusService` straight at `gitxtend.repo_status()` and
delete the per-method round-trips. Both are acceptable; the per-method shim is
the lowest-risk first step.
