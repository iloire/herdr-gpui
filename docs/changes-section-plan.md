# Plan: a Changes section — uncommitted work and recent commits across every repository

Status: draft, not started (2026-10-25).

## Goal

Answer one question at a glance: **"what is going on in each repo I have open?"**
That means the local Git state of every repository the window's workspaces hold, on
every connected host:

- **uncommitted changes**: which files are touched, and how many lines added and removed;
- **recent commits**: the last few on each checkout's branch, with how far it is
  ahead of or behind its upstream;
- **which worktree / agent it belongs to**, because several checkouts of one
  repo are open at once.

The Pull requests and Issues sections are not useful day to day right now, so the
new section should work without GitHub and without a sign-in.

## What already exists (reuse, don't rebuild)

| Piece | Where | What it gives us |
| --- | --- | --- |
| Repo list across hosts | `orchestrator/host.rs` `HerdrWindow::repo_specs` | One spec per Git common dir per host, keyed by `repo_key`. Pull requests, Issues and Security all list from this. |
| A section over every repo | `security/` (`section.rs`, `list.rs`, `fetch.rs`, `saved.rs`) | The best template: background read on an interval plus Refresh, a list, a pane, last read cached in the app cache DB. |
| Activity bar slots | `herdr_control_protocol::ActivityPanel`, `sidebar/activity.rs`, `sidebar/activity/counts.rs` | Label, icon, count bubble. `[sidebar] automations = false` shows how to hide a section. |
| Dirty probe + line counts | `herdr-vcs/src/git.rs` (`Git::track_listed`, `counts`, `parse_numstat`) | Already rate limited, bounded cache, round-robin over listed workspaces. It gives counts, not file lists or commits. |
| Diff of one checkout | `review/` (`open_review`, `ReviewCheckout`) | Clicking a file can open the existing Review tab for that checkout, so we don't need a second diff viewer. |

## Design

### 1. New `ActivityPanel::Changes` section

- Icon in the activity bar (git-commit glyph). The count bubble shows the **number of
  dirty checkouts**.
- Body: one card per repository (grouped like the PR list: per host, then per repo).
  Inside each card, one row per open checkout/worktree:

```
herdr-gpui  ·  local                                   ↻ 12s ago
  main                 ↑0 ↓3   clean
    38ea424  feat(remote): remote control from a phone        2h
    6e51ec0  fix: show avatars for SSH checkouts …            5h
  fix/listening-ports  ↑0 ↓0   +84 −12 · 3 files · 1 new
    M listening_ports/scan.rs           +60 −10
    M listening_ports/tests.rs          +24 −2
    ? listening_ports/tests/renamed_listeners.rs
    38ea424  feat(remote): …                                  2h
```

- Sorted by **what needs attention**: dirty, then ahead of upstream (unpushed),
  then behind, then clean. Clean repos collapse to one line.
- Clicking a file opens the Review tab for that checkout, scrolled to the file.
  ~~Clicking a commit copies its SHA (v1).~~ Clicking a commit opens it in a review tab; a right click copies its SHA ✅. Clicking the checkout row focuses its workspace.
- Filters (pills like Security's): `All · Dirty · Unpushed` plus the search field.

### 2. Data: one read per checkout, off the UI thread

A new read in `herdr-vcs` (it owns Git on checkouts; no GPUI), say `herdr_vcs::overview`.
It runs per checkout:

```sh
git status --porcelain=v2 --branch -z      # branch, upstream, ahead/behind, files
git diff --numstat -z HEAD                 # per-file +/- (reuse parse_numstat)
git log -n 5 --format=%h%x1f%s%x1f%ct%x1f%an -z
```

- Bounded: at most N files per checkout (e.g. 50, then "+12 more"), 5 commits, the
  existing `STATUS_TIMEOUT`, and a concurrency cap.
- Schedule: the focused/visible section refreshes every ~10 s; when the section is
  hidden, only the bar count updates, from the existing dirty probe (no new processes).
- Typed `Error` in `overview/error.rs`, wrapped in the crate root as
  `#[error(transparent)] Overview(#[from] overview::Error)`, per AGENTS.md.
- **SSH hosts**: today's `git.rs` is local-checkout only. The orchestrator already runs
  scripts on SSH hosts (`scripts_run_on`, `repo.rs`). v1 reuses that runner for remote
  checkouts, or ships local only first (see open questions).

### 3. Cache and restart

Same as Security's `saved.rs`: keep the last read in `cache.sqlite3`, show it at start
greyed with "as of …" until the first fresh read replaces it.

### 4. Pull requests / Issues optional

Add `[sidebar] pull_requests = false` and `[sidebar] issues = false`, mirroring
`automations = false`, so the bar can drop sections that aren't useful. They stay on
by default for other users.

### 5. CLI (AGENTS.md requires it)

`herdr-gpui changes [--json] [--dirty]` prints the same per-repo state from the app's
latest read, so an agent can ask "what's uncommitted across my repos?". Also
`herdr-gpui ui panel changes` comes for free from `ActivityPanel`. Document both in `docs/cli.md`
and the managed skill's command table.

## Steps

1. [ ] `herdr-vcs`: `overview` module that parses porcelain v2, numstat and log into a typed
   `CheckoutState { branch, upstream, ahead, behind, files: Vec<FileChange>, commits: Vec<Commit> }`.
   Unit tests against real temporary repos (dirty, untracked, detached, no upstream,
   no commits, rename).
2. [ ] Worker: bounded, rate-limited reads over `repo_specs` checkouts; snapshot through the
   coalescing mailbox.
3. [ ] `ActivityPanel::Changes` in `herdr-control-protocol`, label, icon, count.
4. [ ] `changes/` module in the app (section, list, pane, filter, saved), modelled on `security/`;
   each file stays under 1,000 lines.
5. [ ] Click-through: file → Review tab, checkout row → focus workspace.
6. [ ] `[sidebar] pull_requests` / `issues` toggles + config tests.
7. [ ] CLI `changes` command + docs + skill table.
8. [ ] README "Changes" section + screenshot; `just format && just ci`, plus `just lint-linux`/`lint-windows`.

## Open questions

1. **Add or replace?** Recommended: add Changes as its own section and make PRs/Issues hideable,
   rather than replacing the Pull requests list (the orchestrator dispatch flow lives there).
2. **SSH hosts in v1?** Local first is much smaller; remote needs the orchestrator's script runner.
3. **How many commits?** 5 per checkout by default, configurable later if needed.
4. **Upstream fork:** is this branch of herdr-gpui meant to go upstream as a PR, or stay in your
   copy? It changes how much polish v1 needs (CLI, docs, Windows lint).
