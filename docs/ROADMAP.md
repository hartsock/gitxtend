# gitxtend — Roadmap

## M0 — Scaffold (this commit)
- Apache-2.0 LICENSE + NOTICE.
- README, DESIGN, API contract, PORTING guide.
- Build stubs: `Cargo.toml`, `pyproject.toml`, `src/lib.rs` (signatures +
  `todo!()`), `.pyi` type stubs.
- **Outcome:** you can `git clone`, `maturin develop`, and get an importable
  module whose functions raise `NotImplementedError`/`todo!()`.

## M1 — Read side (the unpushed-work detector)
Implement, with parity tests vs the `git` CLI, in this order:
1. `is_git_repo`, `head_sha`, `current_branch` — smallest, unblock the rest.
2. `tracking_branch`, `remote_head_sha`.
3. `ahead_behind` (+ keep `rev_list_count`), `log_subjects`.
4. `is_clean`, `status_counts`.
5. `remote_urls`, `last_commit_date`.
6. `repo_status()` roll-up + full SyncState tree.
- **Acceptance:** every method agrees with `git` on the fixture matrix;
  `repo_status()` reproduces `check_repo` on diverged/ahead/
  behind/dirty/no-remote/error fixtures.
- **Note:** `fetch()` may ship as a contained shell-out if gix fetch is
  unstable (see PORTING.md). Everything else is pure gix.

## M2 — Plugin adoption
- Add the `GitService` read-method shim (API.md) in the git-tend tool, or
  point the status roll-up straight at `gitxtend.repo_status()`.
- Gate behind a feature flag / env var so it can be rolled back instantly.
- Run the git-tend `scan` / `status` across the real workspace; compare
  output to the subprocess implementation byte-for-byte.

## M3 — Standalone CLI (optional)
- Add a `[[bin]]` target reusing `repo.rs`/`status.rs` so `gitxtend status
  <dir>` works without Python (for cron / shell). Same logic, no PyO3.

## M4 — Write side (only after read side is trusted in prod)
- Evaluate porting `pull --ff-only`, `push`, `add`, `commit`, `stash`,
  `branch`, `reset --hard` to gix. Network/mutation paths are less mature in
  gix; some may stay shelled-out indefinitely. Decide per-method, with the
  same parity-test bar.

## Out of scope (stays in Python, indefinitely)
- CLI/UX, YAML config, forge integration (gh/glab PR/MR auto-merge),
  board conflict resolution, systemd timer.
