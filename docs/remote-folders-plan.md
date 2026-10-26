# Plan: Git and files on every host, through SSH

Status: in progress (2026-10-26).

## Goal

Everything the app reads or writes in a checkout — Git state, the file tree,
a code tab's text, a save — works the same on an SSH host as on this machine.
This is the foundation for centring the app on the folder (a folder home,
scoped Files and Changes, per-folder badges): none of those should ship
local-only and be redone for SSH later.

## Approach: shell only

Nothing is installed on the server. A host needs `/bin/sh` and `git`.

- **One long-lived shell per host**, as listening ports already does
  (`herdr_usage::Shell`, over `ssh … exec /bin/sh -s`, or `wsl.exe`). Each
  Git child or file read is one *step* through it: one round trip, not one
  SSH handshake per Git command.
- **This machine is just another host.** A `herdr_vcs::host::Session` is
  either local (Git children run directly, as today) or a host's shell.
  Readers take a session, so the same code path serves both and a feature
  cannot quietly be local-only.
- **Same parsers.** `git status --porcelain=v2`, `diff --numstat`, `log`,
  `ls-files -z` print the same text over a shell; only how the command
  reaches the host changes.
- **Bounded and off the UI thread.** Steps keep the existing deadlines and
  output limits. A step that overruns breaks that host's shell; the next
  read reconnects, no sooner than a retry delay, so an unreachable host is
  stale, never a stall.
- **Files** are read with the same policy as `code_index::read_source`
  (regular file, not a symlink, at most 1 MiB, no NUL, UTF-8). Saves (step
  4) write a temporary file and `mv` it into place, refused when the file
  changed since it was read.

Not covered: bare-socket targets have no shell to reach; cloud machines are
reached only through their provider's command. Views say so instead of
showing an empty list.

## Steps

1. ~~`herdr-vcs`: `host::Session` (local or a host's shell) with `git`,
   `git_bytes`, and `read_text`; `changes::{locate, worktrees, read}` take a
   session. Tests run the shell path against a local `sh` standing in for a
   remote one, on real temporary repositories.~~ ✅
2. [ ] Changes section: discover and read the checkouts of every SSH host's
   workspaces through that host's session, grouped by host.
3. [ ] Files view: `code_index::Index::list` through a session, so the tree
   follows a pane on an SSH host.
4. [ ] Code tabs: open, follow edits on disk (`stat` polling), and save
   (temporary file + `mv`, refused on conflict) through a session.
5. [ ] Review diffs and the titlebar Git chip through a session.
6. [ ] Docs: README and `docs/cli.md` say which views reach SSH hosts.

## Open questions

- Push from a remote checkout uses the server's own Git credentials. When it
  has none the push fails with Git's message; the app never copies a token
  there without asking.
