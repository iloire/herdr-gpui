//! Finding the checkouts a pass reads, then reading them, all on the
//! background executor. Seeds come from the open workspaces and panes,
//! whose folders may be anywhere inside a checkout or in none, and from the
//! config. Each seed is resolved to its checkout, and each repository found
//! adds its other worktrees. A seed outside Git is no news; only what the
//! config names reports a failure.

use super::{Failed, Read, Shown, Workspace, group};
use herdr_vcs::{
    changes::{self, Located},
    host::Session,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

/// One checkout's Git children together, as the status chip allows.
const TIMEOUT: Duration = Duration::from_secs(15);
/// Checkouts read per pass, however many the seeds lead to.
pub(crate) const MAX_CHECKOUTS: usize = 128;

/// Where a pass looks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Targets {
    /// Open folders, each with the workspaces it is open in, in the order
    /// first seen.
    pub(crate) open: Vec<(String, Vec<Workspace>)>,
    /// Checkouts `[changes] repos` names.
    pub(crate) repos: Vec<PathBuf>,
    /// Folders `[changes] roots` names.
    pub(crate) roots: Vec<PathBuf>,
}

impl Targets {
    /// Adds `directory` as open in `workspace`. Empty and relative folders
    /// name nothing this machine can read.
    pub(crate) fn add_open(&mut self, directory: &str, workspace: &Workspace) {
        if directory.is_empty() || !std::path::Path::new(directory).is_absolute() {
            return;
        }
        match self.open.iter_mut().find(|(seen, _)| seen == directory) {
            Some((_, workspaces)) if !workspaces.contains(workspace) => {
                workspaces.push(workspace.clone());
            }
            Some(_) => {}
            None => self
                .open
                .push((directory.to_owned(), vec![workspace.clone()])),
        }
    }
}

/// The checkouts found so far, each with the workspaces open in it.
#[derive(Default)]
struct Found {
    checkouts: Vec<(Located, Vec<Workspace>)>,
    failed: Vec<Failed>,
}

impl Found {
    fn add(&mut self, located: Located, workspaces: &[Workspace]) {
        let index = match self
            .checkouts
            .iter()
            .position(|(seen, _)| seen.path == located.path)
        {
            Some(index) => index,
            None => {
                self.checkouts.push((located, Vec::new()));
                self.checkouts.len() - 1
            }
        };
        let entry = &mut self.checkouts[index].1;
        for workspace in workspaces {
            if !entry.contains(workspace) {
                entry.push(workspace.clone());
            }
        }
    }

    fn knows(&self, path: &str) -> bool {
        self.checkouts.iter().any(|(seen, _)| seen.path == path)
    }
}

/// Finds every checkout `targets` leads to and reads it, stopping early
/// once `cancelled` says so.
pub(crate) fn discover(targets: &Targets, cancelled: &impl Fn() -> bool) -> Read {
    let deadline = || Instant::now() + TIMEOUT;
    let session = &mut Session::local();
    let mut found = Found::default();
    for (directory, workspaces) in &targets.open {
        if cancelled() {
            return Read::default();
        }
        if let Ok(located) = changes::locate(session, directory, deadline(), cancelled) {
            found.add(located, workspaces);
        }
    }
    let scanned = targets
        .roots
        .iter()
        .flat_map(|root| changes::repositories_in(root));
    let named = targets
        .repos
        .iter()
        .filter_map(|path| path.to_str().map(str::to_owned));
    for path in named.chain(scanned) {
        if cancelled() {
            return Read::default();
        }
        match changes::locate(session, &path, deadline(), cancelled) {
            Ok(located) => found.add(located, &[]),
            Err(error) => found.failed.push(Failed { label: path, error }),
        }
    }
    // Every repository adds its other worktrees, once, from whichever of
    // its checkouts was found first.
    let mut repositories: Vec<(String, String)> = Vec::new();
    for (located, _) in &found.checkouts {
        if !repositories
            .iter()
            .any(|(common, _)| *common == located.common_dir)
        {
            repositories.push((located.common_dir.clone(), located.path.clone()));
        }
    }
    for (_, path) in repositories {
        if cancelled() {
            return Read::default();
        }
        let Ok(worktrees) = changes::worktrees(session, &path, deadline(), cancelled) else {
            continue;
        };
        for worktree in worktrees {
            if found.knows(&worktree) {
                continue;
            }
            if let Ok(located) = changes::locate(session, &worktree, deadline(), cancelled) {
                found.add(located, &[]);
            }
        }
    }
    let mut shown = Vec::new();
    let mut failed = found.failed;
    for (located, workspaces) in found.checkouts.into_iter().take(MAX_CHECKOUTS) {
        if cancelled() {
            return Read::default();
        }
        let label = located.path.clone();
        match changes::read(session, located, deadline(), cancelled) {
            Ok(checkout) => shown.push(Shown {
                checkout,
                workspaces,
            }),
            Err(error) => failed.push(Failed { label, error }),
        }
    }
    Read {
        repos: group(shown),
        failed,
    }
}
