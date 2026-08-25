//! Submodule helpers.
//!
//! Unlike most v1 methods (which are pure gix), submodule update/status is
//! intentionally delegated to the local `git` CLI so we follow Git's own
//! submodule semantics exactly.
//!
//! # Why an update needs three steps, not one
//!
//! `git submodule update --remote` moves each submodule's working tree to the
//! tip of the branch it tracks — and stops there. It leaves behind:
//!
//! - a **detached HEAD** inside each submodule, and
//! - a **modified gitlink** in the superproject index.
//!
//! So on its own it makes the superproject dirty rather than up to date: the
//! next plain `git submodule update` snaps every submodule back to the SHA the
//! superproject still records. Recording the bumps is what makes the update
//! durable, and it is a separate commit in the superproject.
//!
//! [`update_submodules`] is the whole loop: snapshot → update → diff the
//! snapshots → optionally record. The diff is what lets a caller say *which*
//! submodules moved and from where, which `git submodule update` never reports.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// Commit subject used when a caller records gitlink bumps without supplying
/// their own message.
const DEFAULT_COMMIT_SUBJECT: &str = "chore: update submodules to tracked branch tips";

/// One row from `git submodule status`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmoduleStatusEntry {
    /// Submodule path inside the superproject.
    pub path: String,
    /// One of `clean`, `not-initialized`, `out-of-date`, `unmerged`, `unknown`.
    pub state: String,
    /// Reported commit SHA (or placeholder hash from the superproject index).
    pub commit: String,
    /// Optional extra detail (usually `(branch)`).
    pub detail: String,
}

/// One submodule that moved during [`update_submodules`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmoduleChange {
    /// Submodule path inside the superproject.
    pub path: String,
    /// Commit the submodule sat at before the update.
    pub from: String,
    /// Commit it sits at after the update.
    pub to: String,
    /// The submodule had no working tree before this run (`-` in status) and
    /// this run checked it out. `from` is then the SHA the superproject
    /// recorded, not one that was ever checked out.
    pub initialized: bool,
    /// Branch this submodule tracks, from `.gitmodules`. Empty when none is
    /// declared — git then follows the remote's default branch.
    pub branch: String,
}

/// How [`update_submodules`] should behave.
///
/// [`Default`] is the "keep every submodule on the tip of the branch it tracks"
/// configuration — recursive, remote-tracking, and **not** committing.
#[derive(Clone, Debug)]
pub struct UpdateOptions {
    /// Recurse into nested submodules (`--recursive`).
    pub recursive: bool,
    /// Move to the tip of each submodule's tracked branch (`--remote`) rather
    /// than to the SHA the superproject already records. This is the flag that
    /// makes the operation an *update* instead of a *restore*.
    pub remote: bool,
    /// Record the resulting gitlink bumps as a commit in the superproject.
    pub commit: bool,
    /// Commit message; [`DEFAULT_COMMIT_SUBJECT`] plus a per-submodule body
    /// when `None`. Ignored unless `commit` is set.
    pub message: Option<String>,
}

impl Default for UpdateOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            remote: true,
            commit: false,
            message: None,
        }
    }
}

/// What [`update_submodules`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateReport {
    /// Every step that ran succeeded.
    pub ok: bool,
    /// Submodules whose checked-out commit moved, or that this run initialized.
    /// Empty means everything was already current — the operation is idempotent.
    pub changed: Vec<SubmoduleChange>,
    /// Superproject commit recording the bumps, when [`UpdateOptions::commit`]
    /// was set and there was something to record.
    pub commit: Option<String>,
    /// Diagnostics from the first failing step. Empty on a clean run.
    pub stderr: String,
}

fn status_state(marker: char) -> &'static str {
    match marker {
        ' ' => "clean",
        '-' => "not-initialized",
        '+' => "out-of-date",
        'U' => "unmerged",
        _ => "unknown",
    }
}

fn parse_status_line(line: &str) -> Option<SubmoduleStatusEntry> {
    if line.is_empty() {
        return None;
    }

    let mut chars = line.chars();
    let marker = chars.next()?;
    let remainder = chars.as_str().trim_start();

    let mut pieces = remainder.splitn(3, ' ');
    let commit = pieces.next()?.trim();
    let path = pieces.next()?.trim();
    let detail = pieces.next().unwrap_or("").trim();

    if commit.is_empty() || path.is_empty() {
        return None;
    }

    Some(SubmoduleStatusEntry {
        path: path.to_string(),
        state: status_state(marker).to_string(),
        commit: commit.to_string(),
        detail: detail.to_string(),
    })
}

/// Map submodule path -> the branch it tracks, as declared in `.gitmodules`.
///
/// A submodule with no `branch =` line is **absent from the map**: git then
/// follows the remote's default branch, which `.gitmodules` does not name. That
/// absence is the honest answer, and is why this is not read out of
/// `git submodule status`'s trailing detail — after a `--remote` update that
/// detail reads `(remotes/origin/HEAD)` for *every* submodule, including ones
/// that track a named branch, because it describes the commit rather than the
/// configured tracking.
///
/// Only the superproject's own `.gitmodules` is read, so with `recursive` the
/// nested submodules (whose declarations live one level down) get no branch.
fn tracked_branches(path: &Path) -> BTreeMap<String, String> {
    let listing = match run(git_in(
        path,
        &[
            "config",
            "-f",
            ".gitmodules",
            "--get-regexp",
            r"^submodule\..*\.(path|branch)$",
        ],
    )) {
        Ok(out) => out,
        // No `.gitmodules`, or no matching keys: git exits non-zero.
        Err(_) => return BTreeMap::new(),
    };

    let mut paths: BTreeMap<String, String> = BTreeMap::new();
    let mut branches: BTreeMap<String, String> = BTreeMap::new();
    for line in listing.lines() {
        let Some((key, value)) = line.split_once(' ') else {
            continue;
        };
        // `submodule.<name>.path`, where <name> may itself contain dots.
        let Some((name, field)) = key
            .strip_prefix("submodule.")
            .and_then(|rest| rest.rsplit_once('.'))
        else {
            continue;
        };
        match field {
            "path" => {
                paths.insert(name.to_string(), value.to_string());
            }
            "branch" => {
                branches.insert(name.to_string(), value.to_string());
            }
            _ => {}
        }
    }

    paths
        .into_iter()
        .filter_map(|(name, sub_path)| branches.get(&name).map(|b| (sub_path, b.clone())))
        .collect()
}

/// A `git` invocation in `path` with the ambient repo env scrubbed, so a caller
/// running under `git` hooks (where `GIT_DIR` et al. point at *their* repo)
/// still operates on `path`.
fn git_in(path: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(path)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    cmd
}

/// Run a command, returning trimmed stdout on success and trimmed stderr as the
/// error on failure.
fn run(mut cmd: Command) -> Result<String, String> {
    match cmd.output() {
        Ok(out) if out.status.success() => {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
        Ok(out) => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Parse `git submodule status` output into structured rows.
/// Returns `[]` on any command/path error (soft-fail behavior, matching v1 style).
pub fn submodule_status(path: &Path, recursive: bool) -> Vec<SubmoduleStatusEntry> {
    let mut cmd = git_in(path, &["submodule", "status"]);
    if recursive {
        cmd.arg("--recursive");
    }

    let output = match cmd.output() {
        Ok(out) if out.status.success() => out,
        Ok(_) | Err(_) => return Vec::new(),
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_status_line)
        .collect()
}

/// Run `git submodule update --init` for the repo.
/// Returns `(ok, stderr)` so caller can surface errors.
///
/// This is the raw primitive. It reports nothing about *what* moved and leaves
/// the superproject holding modified gitlinks — see [`update_submodules`] for
/// the complete operation.
pub fn sync_submodules(path: &Path, recursive: bool, update_remote: bool) -> (bool, String) {
    let mut cmd = git_in(path, &["submodule", "update", "--init"]);
    if recursive {
        cmd.arg("--recursive");
    }
    if update_remote {
        cmd.arg("--remote");
    }

    match cmd.output() {
        Ok(out) => (
            out.status.success(),
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ),
        Err(e) => (false, e.to_string()),
    }
}

fn by_path(entries: Vec<SubmoduleStatusEntry>) -> BTreeMap<String, SubmoduleStatusEntry> {
    entries
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect()
}

/// Build the default commit message: a fixed subject plus one body line per
/// submodule, so the bump commit says what it bumped.
fn default_message(changed: &[SubmoduleChange]) -> String {
    let mut msg = String::from(DEFAULT_COMMIT_SUBJECT);
    msg.push_str("\n\n");
    for change in changed {
        msg.push_str(&format!(
            "{}: {} -> {}\n",
            change.path,
            short(&change.from),
            short(&change.to)
        ));
    }
    msg
}

fn short(sha: &str) -> &str {
    if sha.len() > 7 {
        &sha[..7]
    } else {
        sha
    }
}

/// Stage the moved gitlinks and commit them in the superproject.
///
/// Only **top-level** submodules are staged. With `--recursive`, a nested
/// submodule shows up in status as `outer/inner`, which the superproject cannot
/// stage — that path lives inside `outer`, and recording it requires a commit
/// *in* `outer` first. Attempting it would fail with "Pathspec is in
/// submodule", so nested bumps are deliberately left for the caller.
fn commit_bumps(
    path: &Path,
    changed: &[SubmoduleChange],
    message: Option<&str>,
) -> Result<Option<String>, String> {
    let top_level: Vec<&SubmoduleChange> = {
        let tops = by_path(submodule_status(path, false));
        changed
            .iter()
            .filter(|change| tops.contains_key(&change.path))
            .collect()
    };
    if top_level.is_empty() {
        return Ok(None);
    }

    let mut add = git_in(path, &["add", "--"]);
    for change in &top_level {
        add.arg(&change.path);
    }
    run(add)?;

    let owned: Vec<SubmoduleChange> = top_level.into_iter().cloned().collect();
    let msg = message
        .map(str::to_string)
        .unwrap_or_else(|| default_message(&owned));
    run(git_in(path, &["commit", "-m", &msg]))?;

    run(git_in(path, &["rev-parse", "HEAD"])).map(Some)
}

/// Update every submodule to the tip of the branch it tracks, report what
/// moved, and optionally record the bumps in the superproject.
///
/// This is the whole "keep this repo full of submodules up to date" operation:
///
/// 1. snapshot `git submodule status`,
/// 2. `git submodule update --init [--recursive] [--remote]`,
/// 3. snapshot again and diff — that diff is [`UpdateReport::changed`],
/// 4. with [`UpdateOptions::commit`], stage the moved top-level gitlinks and
///    commit them, so the update survives the next checkout.
///
/// Idempotent: a second run with nothing to do reports `changed == []`, and the
/// commit step is skipped rather than producing an empty commit.
///
/// Soft-fails like the rest of the v1 surface — a failure sets
/// [`UpdateReport::ok`] to `false` and fills `stderr` rather than panicking.
pub fn update_submodules(path: &Path, opts: &UpdateOptions) -> UpdateReport {
    let before = by_path(submodule_status(path, opts.recursive));

    let (ok, stderr) = sync_submodules(path, opts.recursive, opts.remote);
    if !ok {
        return UpdateReport {
            ok: false,
            changed: Vec::new(),
            commit: None,
            stderr,
        };
    }

    let branches = tracked_branches(path);
    let changed: Vec<SubmoduleChange> = submodule_status(path, opts.recursive)
        .into_iter()
        .filter_map(|entry| {
            let prior = before.get(&entry.path);
            let from = prior.map(|p| p.commit.clone()).unwrap_or_default();
            let initialized = prior.is_some_and(|p| p.state == "not-initialized");
            // A submodule counts as changed when its commit moved OR when this
            // run checked it out for the first time (which can happen at an
            // unchanged SHA, and is still news to the caller).
            if from == entry.commit && !initialized {
                return None;
            }
            let branch = branches.get(&entry.path).cloned().unwrap_or_default();
            Some(SubmoduleChange {
                path: entry.path,
                from,
                to: entry.commit,
                initialized,
                branch,
            })
        })
        .collect();

    let mut report = UpdateReport {
        ok: true,
        changed,
        commit: None,
        stderr,
    };

    if opts.commit && !report.changed.is_empty() {
        match commit_bumps(path, &report.changed, opts.message.as_deref()) {
            Ok(sha) => report.commit = sha,
            Err(e) => {
                report.ok = false;
                report.stderr = e;
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::fixtures;
    use std::fs;
    use tempfile::TempDir;

    /// A superproject with one submodule, wired so the production code path can
    /// actually fetch.
    ///
    /// `protocol.file.allow` is set in the *fixture's local config* rather than
    /// passed as a `-c` override, because the code under test builds its own
    /// `git` invocations. Overriding there would test a command we do not ship.
    struct Fixture {
        parent: TempDir,
        child: TempDir,
    }

    impl Fixture {
        /// `branch` is the branch the submodule tracks; `None` leaves
        /// `.gitmodules` without a `branch =` line, so `--remote` falls back to
        /// the remote's `HEAD`.
        fn new(branch: Option<&str>) -> Self {
            let parent = fixtures::repo();
            let child = tempfile::tempdir().expect("tempdir");

            fixtures::git(child.path(), &["init", "-q", "-b", "main"]);
            fixtures::write(child.path(), "f.txt", "v1");
            fixtures::git(child.path(), &["add", "-A"]);
            fixtures::git(child.path(), &["commit", "-q", "-m", "child v1"]);
            if let Some(b) = branch {
                fixtures::git(child.path(), &["checkout", "-q", "-b", b]);
                fixtures::write(child.path(), "f.txt", "on-branch");
                fixtures::git(child.path(), &["commit", "-q", "-am", "child branch"]);
            }

            // The superproject must allow the file transport for submodule
            // fetches, and must carry an identity for the `--commit` path.
            fixtures::git(parent.path(), &["config", "protocol.file.allow", "always"]);
            fixtures::git(parent.path(), &["config", "user.name", "fix"]);
            fixtures::git(parent.path(), &["config", "user.email", "fix@example.com"]);

            let url = child.path().to_str().expect("utf-8 path");
            let mut add = vec!["-c", "protocol.file.allow=always", "submodule", "add", "-q"];
            if let Some(b) = branch {
                add.push("-b");
                add.push(b);
            }
            add.push(url);
            add.push("mod");
            fixtures::git(parent.path(), &add);
            fixtures::git(parent.path(), &["add", "-A"]);
            fixtures::git(parent.path(), &["commit", "-q", "-m", "add submodule"]);

            // `--remote` fetches from *inside* the submodule, which reads the
            // submodule's own config, so the parent's setting does not cover it.
            fixtures::git(
                &parent.path().join("mod"),
                &["config", "protocol.file.allow", "always"],
            );

            Self { parent, child }
        }

        /// Add a commit to the child on `branch` (its current branch if `None`)
        /// so the tracked tip moves ahead of what the superproject records.
        fn advance_child(&self, branch: Option<&str>) -> String {
            if let Some(b) = branch {
                fixtures::git(self.child.path(), &["checkout", "-q", b]);
            }
            fixtures::write(self.child.path(), "f.txt", "advanced");
            fixtures::git(self.child.path(), &["commit", "-q", "-am", "child advance"]);
            fixtures::git(self.child.path(), &["rev-parse", "HEAD"])
        }

        fn parent_path(&self) -> &Path {
            self.parent.path()
        }

        fn porcelain(&self) -> String {
            fixtures::git(self.parent_path(), &["status", "--porcelain"])
        }
    }

    #[test]
    fn parse_status_state_markers() {
        let cases = vec![
            (" 1234567890abcdef1234567890abcdef12345678 path", "clean"),
            (
                "-1234567890abcdef1234567890abcdef12345678 path",
                "not-initialized",
            ),
            (
                "+1234567890abcdef1234567890abcdef12345678 path (main)",
                "out-of-date",
            ),
            ("U1234567890abcdef1234567890abcdef12345678 path", "unmerged"),
            ("?1234567890abcdef1234567890abcdef12345678 path", "unknown"),
        ];

        for (line, expected) in cases {
            let entry = parse_status_line(line).expect("parseable");
            assert_eq!(entry.state, expected);
            assert_eq!(entry.path, "path");
            assert_eq!(entry.commit, "1234567890abcdef1234567890abcdef12345678");
        }
    }

    #[test]
    fn tracked_branches_reads_gitmodules_not_the_status_detail() {
        let fx = Fixture::new(Some("devel"));
        assert_eq!(
            tracked_branches(fx.parent_path())
                .get("mod")
                .map(String::as_str),
            Some("devel")
        );

        // A submodule with no `branch =` line is absent, not guessed at.
        let plain = Fixture::new(None);
        assert_eq!(tracked_branches(plain.parent_path()).get("mod"), None);

        // And a repo with no submodules at all yields an empty map, not an error.
        let bare = fixtures::repo();
        assert!(tracked_branches(bare.path()).is_empty());
    }

    #[test]
    fn change_reports_the_configured_branch_even_after_a_remote_update() {
        // Regression guard: `git submodule status` describes the *commit*, so
        // after `--remote` its detail reads `(remotes/origin/HEAD)` for a
        // submodule that actually tracks `devel`. The report must say `devel`.
        let fx = Fixture::new(Some("devel"));
        fx.advance_child(Some("devel"));

        let report = update_submodules(fx.parent_path(), &UpdateOptions::default());
        assert!(report.ok, "{}", report.stderr);
        assert_eq!(report.changed[0].branch, "devel");

        let detail = &submodule_status(fx.parent_path(), false)[0].detail;
        assert!(
            detail.contains("origin/HEAD"),
            "precondition: git's own detail is the misleading one, got {detail}"
        );
    }

    #[test]
    fn submodule_status_reports_not_initialized_and_clean() {
        let fx = Fixture::new(None);

        let rows_clean = submodule_status(fx.parent_path(), false);
        assert_eq!(rows_clean.len(), 1);
        assert_eq!(rows_clean[0].state, "clean");
        assert_eq!(rows_clean[0].path, "mod");

        fs::remove_dir_all(fx.parent_path().join("mod")).unwrap();
        let rows_missing = submodule_status(fx.parent_path(), false);
        assert_eq!(rows_missing.len(), 1);
        assert_eq!(rows_missing[0].state, "not-initialized");
    }

    #[test]
    fn sync_submodules_reinitializes_submodule() {
        let fx = Fixture::new(None);

        fs::remove_dir_all(fx.parent_path().join("mod")).unwrap();
        let (ok, stderr) = sync_submodules(fx.parent_path(), false, false);
        assert!(ok, "{stderr}");
        assert!(fx.parent_path().join("mod").exists());
        assert_eq!(submodule_status(fx.parent_path(), false)[0].state, "clean");
    }

    #[test]
    fn sync_submodules_on_repo_with_no_submodules_still_ok() {
        let td = fixtures::repo();
        assert!(sync_submodules(td.path(), false, false).0);
    }

    #[test]
    fn update_submodules_advances_to_the_tracked_branch_tip() {
        // The headline use case: the submodule tracks `devel`, `devel` gains a
        // commit, and one call moves the submodule onto it and says so.
        let fx = Fixture::new(Some("devel"));
        let before = submodule_status(fx.parent_path(), false)[0].commit.clone();
        let tip = fx.advance_child(Some("devel"));

        let report = update_submodules(fx.parent_path(), &UpdateOptions::default());

        assert!(report.ok, "{}", report.stderr);
        assert_eq!(report.changed.len(), 1, "{:?}", report.changed);
        let change = &report.changed[0];
        assert_eq!(change.path, "mod");
        assert_eq!(change.from, before);
        assert_eq!(change.to, tip, "must land on the tip of the tracked branch");
        assert!(!change.initialized);
        // Not committed by default, so the bump is left staged in the worktree.
        assert_eq!(report.commit, None);
        assert!(
            fx.porcelain().contains("mod"),
            "an uncommitted run must leave the gitlink modified"
        );
    }

    #[test]
    fn update_submodules_without_remote_does_not_advance() {
        // `remote: false` is a *restore* (to the recorded SHA), not an update.
        // This is the flag that separates the two, so pin it.
        let fx = Fixture::new(Some("devel"));
        let recorded = submodule_status(fx.parent_path(), false)[0].commit.clone();
        fx.advance_child(Some("devel"));

        let report = update_submodules(
            fx.parent_path(),
            &UpdateOptions {
                remote: false,
                ..UpdateOptions::default()
            },
        );

        assert!(report.ok, "{}", report.stderr);
        assert!(
            report.changed.is_empty(),
            "without --remote nothing should move: {:?}",
            report.changed
        );
        assert_eq!(
            submodule_status(fx.parent_path(), false)[0].commit,
            recorded
        );
    }

    #[test]
    fn update_submodules_commit_records_the_bump_and_leaves_a_clean_tree() {
        let fx = Fixture::new(Some("devel"));
        let head_before = fixtures::git(fx.parent_path(), &["rev-parse", "HEAD"]);
        let tip = fx.advance_child(Some("devel"));

        let report = update_submodules(
            fx.parent_path(),
            &UpdateOptions {
                commit: true,
                ..UpdateOptions::default()
            },
        );

        assert!(report.ok, "{}", report.stderr);
        let sha = report.commit.expect("a bump must be recorded");
        assert_ne!(sha, head_before, "the superproject must have a new commit");
        assert_eq!(
            fixtures::git(fx.parent_path(), &["rev-parse", "HEAD"]),
            sha,
            "the reported sha must be the superproject HEAD"
        );
        assert_eq!(
            fx.porcelain(),
            "",
            "recording the bump is what leaves the superproject clean"
        );
        // And the recorded gitlink is the new tip, not the old one.
        assert_eq!(
            fixtures::git(fx.parent_path(), &["rev-parse", "HEAD:mod"]),
            tip
        );
    }

    #[test]
    fn update_submodules_default_commit_message_names_what_moved() {
        let fx = Fixture::new(Some("devel"));
        fx.advance_child(Some("devel"));

        let report = update_submodules(
            fx.parent_path(),
            &UpdateOptions {
                commit: true,
                ..UpdateOptions::default()
            },
        );
        assert!(report.ok, "{}", report.stderr);

        let body = fixtures::git(fx.parent_path(), &["log", "-1", "--pretty=%B"]);
        assert!(body.contains(DEFAULT_COMMIT_SUBJECT), "{body}");
        assert!(
            body.contains("mod: "),
            "the body must name the submodule that moved: {body}"
        );
    }

    #[test]
    fn update_submodules_honors_a_custom_commit_message() {
        let fx = Fixture::new(Some("devel"));
        fx.advance_child(Some("devel"));

        let report = update_submodules(
            fx.parent_path(),
            &UpdateOptions {
                commit: true,
                message: Some("bump: my own subject".to_string()),
                ..UpdateOptions::default()
            },
        );
        assert!(report.ok, "{}", report.stderr);
        assert_eq!(
            fixtures::git(fx.parent_path(), &["log", "-1", "--pretty=%s"]),
            "bump: my own subject"
        );
    }

    #[test]
    fn update_submodules_is_idempotent_and_makes_no_empty_commit() {
        let fx = Fixture::new(Some("devel"));
        fx.advance_child(Some("devel"));

        let first = update_submodules(
            fx.parent_path(),
            &UpdateOptions {
                commit: true,
                ..UpdateOptions::default()
            },
        );
        assert!(first.ok, "{}", first.stderr);
        assert_eq!(first.changed.len(), 1);
        let head_after_first = fixtures::git(fx.parent_path(), &["rev-parse", "HEAD"]);

        let second = update_submodules(
            fx.parent_path(),
            &UpdateOptions {
                commit: true,
                ..UpdateOptions::default()
            },
        );

        assert!(second.ok, "{}", second.stderr);
        assert!(
            second.changed.is_empty(),
            "a repeat run has nothing to do: {:?}",
            second.changed
        );
        assert_eq!(second.commit, None, "no empty commit may be created");
        assert_eq!(
            fixtures::git(fx.parent_path(), &["rev-parse", "HEAD"]),
            head_after_first,
            "the superproject HEAD must not move on a no-op run"
        );
    }

    #[test]
    fn update_submodules_reports_a_checkout_as_initialized() {
        let fx = Fixture::new(None);
        fs::remove_dir_all(fx.parent_path().join("mod")).unwrap();
        assert_eq!(
            submodule_status(fx.parent_path(), false)[0].state,
            "not-initialized"
        );

        // No new upstream commit: the SHA does not move, but the checkout is
        // still news — otherwise a first-time init reports "nothing happened".
        let report = update_submodules(
            fx.parent_path(),
            &UpdateOptions {
                remote: false,
                ..UpdateOptions::default()
            },
        );

        assert!(report.ok, "{}", report.stderr);
        assert_eq!(report.changed.len(), 1, "{:?}", report.changed);
        assert!(report.changed[0].initialized);
        assert!(fx.parent_path().join("mod").exists());
    }

    #[test]
    fn update_submodules_on_repo_with_no_submodules_is_a_clean_no_op() {
        let td = fixtures::repo();
        let report = update_submodules(td.path(), &UpdateOptions::default());
        assert!(report.ok, "{}", report.stderr);
        assert!(report.changed.is_empty());
        assert_eq!(report.commit, None);
    }

    #[test]
    fn update_submodules_reports_failure_instead_of_panicking() {
        let td = tempfile::tempdir().expect("tempdir");
        // Not a git repo at all: every git call fails.
        let report = update_submodules(td.path(), &UpdateOptions::default());
        assert!(!report.ok);
        assert!(!report.stderr.is_empty(), "the failure must be surfaced");
    }
}
