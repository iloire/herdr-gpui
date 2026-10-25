//! The activity bar's Changes section: what every workspace on this machine
//! holds in Git right now. Each checkout lists its uncommitted files with
//! their line counts, how far its branch is from its upstream, and its
//! latest commits, grouped by repository so a repository's worktrees sit
//! together. What needs the user leads: dirty checkouts, then unpushed, then
//! behind, then clean.
//!
//! Only checkouts of this machine's own daemon are read, the same trust
//! boundary the titlebar's Git chip keeps. A background read runs every
//! [`SHOWN`] while the section is on screen and every [`HIDDEN`] otherwise,
//! so the bar's count stays current; drawing reads what it left.

mod section;

#[cfg(test)]
mod tests;

use crate::{
    HerdrWindow,
    pull_request::{Input, Origin, workspace_input},
    sidebar::ActivityPanel,
};
use gpui::{prelude::*, *};
use herdr_vcs::changes::{self, Checkout};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

/// How often to read again while the section is shown.
const SHOWN: Duration = Duration::from_secs(10);
/// How often to read again for the bar's count alone.
const HIDDEN: Duration = Duration::from_secs(60);
/// One checkout's Git children together, as the status chip allows.
const TIMEOUT: Duration = Duration::from_secs(15);
/// Checkouts read per pass, however many workspaces the daemon lists.
const MAX_CHECKOUTS: usize = 64;

/// A workspace showing a checkout; several may share one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Workspace {
    pub(crate) id: String,
    pub(crate) label: String,
}

/// A checkout as last read, and the workspaces in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Shown {
    pub(crate) checkout: Checkout,
    pub(crate) workspaces: Vec<Workspace>,
}

impl Shown {
    fn rank(&self) -> Rank {
        rank(&self.checkout)
    }
}

/// One repository: its checkouts, most needing first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Repo {
    pub(crate) name: String,
    pub(crate) checkouts: Vec<Shown>,
}

/// What a checkout asks of the user, most pressing first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rank {
    Dirty,
    Unpushed,
    Behind,
    Clean,
}

pub(crate) fn rank(checkout: &Checkout) -> Rank {
    if checkout.dirty() {
        Rank::Dirty
    } else if checkout.ahead > 0 {
        Rank::Unpushed
    } else if checkout.behind > 0 {
        Rank::Behind
    } else {
        Rank::Clean
    }
}

/// A workspace whose checkout could not be read, and why.
#[derive(Debug)]
pub(crate) struct Failed {
    pub(crate) workspace: Workspace,
    pub(crate) error: herdr_vcs::Error,
}

/// What one pass read.
#[derive(Debug, Default)]
pub(crate) struct Read {
    pub(crate) repos: Vec<Repo>,
    pub(crate) failed: Vec<Failed>,
}

/// The section's state in a window.
pub(crate) struct Changes {
    read: Read,
    /// When the last read finished, for "Updated … ago".
    read_at: Option<SystemTime>,
    /// When the next read is due; `None` reads at the next tick.
    next: Option<Instant>,
    task: Option<Task<()>>,
    /// The checkouts the last read was asked for, so a workspace opened or
    /// closed reads again at once rather than at the next interval.
    inputs: Vec<Input>,
    /// Tells a read in flight to stop when the window goes.
    cancelled: Arc<AtomicBool>,
    scroll: ScrollHandle,
}

impl Default for Changes {
    fn default() -> Self {
        Self {
            read: Read::default(),
            read_at: None,
            next: None,
            task: None,
            inputs: Vec::new(),
            cancelled: Arc::default(),
            scroll: ScrollHandle::new(),
        }
    }
}

impl Drop for Changes {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Changes {
    pub(crate) fn repos(&self) -> &[Repo] {
        &self.read.repos
    }

    pub(crate) fn failed(&self) -> &[Failed] {
        &self.read.failed
    }

    pub(crate) fn read_at(&self) -> Option<SystemTime> {
        self.read_at
    }

    pub(crate) fn busy(&self) -> bool {
        self.task.is_some()
    }

    /// Checkouts with uncommitted work: the bar's count.
    pub(crate) fn dirty(&self) -> usize {
        self.read
            .repos
            .iter()
            .flat_map(|repo| &repo.checkouts)
            .filter(|shown| shown.checkout.dirty())
            .count()
    }

    /// Reads again at the next tick.
    pub(crate) fn refresh(&mut self) {
        self.next = None;
    }

    fn due(&self, now: Instant) -> bool {
        self.task.is_none() && self.next.is_none_or(|next| now >= next)
    }

    fn install(&mut self, read: Read, next: Instant, at: SystemTime) {
        self.read = read;
        self.read_at = Some(at);
        self.next = Some(next);
        self.task = None;
    }

    /// Installs `read` as a pass would, for tests outside this module.
    #[cfg(test)]
    pub(crate) fn read_for_test(&mut self, read: Read) {
        let now = Instant::now();
        self.install(read, now + HIDDEN, SystemTime::now());
    }
}

/// Groups what each workspace's read found by repository: workspaces in one
/// checkout become one entry, and repositories and their checkouts are
/// ordered by what they ask of the user, then by name and path.
pub(crate) fn group(reads: Vec<(Workspace, Result<Checkout, herdr_vcs::Error>)>) -> Read {
    let mut read = Read::default();
    let mut shown: Vec<Shown> = Vec::new();
    for (workspace, result) in reads {
        match result {
            Ok(checkout) => match shown
                .iter_mut()
                .find(|shown| shown.checkout.path == checkout.path)
            {
                Some(existing) => existing.workspaces.push(workspace),
                None => shown.push(Shown {
                    checkout,
                    workspaces: vec![workspace],
                }),
            },
            Err(error) => read.failed.push(Failed { workspace, error }),
        }
    }
    let mut repos: Vec<(String, Repo)> = Vec::new();
    for entry in shown {
        let common = entry.checkout.common_dir.clone();
        match repos.iter_mut().find(|(key, _)| *key == common) {
            Some((_, repo)) => repo.checkouts.push(entry),
            None => repos.push((
                common.clone(),
                Repo {
                    name: repo_name(&common),
                    checkouts: vec![entry],
                },
            )),
        }
    }
    let mut repos: Vec<Repo> = repos.into_iter().map(|(_, repo)| repo).collect();
    for repo in &mut repos {
        repo.checkouts
            .sort_by(|a, b| (a.rank(), &a.checkout.path).cmp(&(b.rank(), &b.checkout.path)));
    }
    repos.sort_by(|a, b| {
        let first = |repo: &Repo| repo.checkouts.first().map_or(Rank::Clean, Shown::rank);
        (first(a), a.name.to_lowercase()).cmp(&(first(b), b.name.to_lowercase()))
    });
    read.repos = repos;
    read
}

/// A repository's name from its common directory: the folder holding
/// `.git`, or a bare repository's own folder without its `.git` suffix.
pub(crate) fn repo_name(common_dir: &str) -> String {
    let path = Path::new(common_dir);
    let folder = if path.file_name().is_some_and(|name| name == ".git") {
        path.parent()
    } else {
        Some(path)
    };
    folder
        .and_then(Path::file_name)
        .map(|name| {
            let name = name.to_string_lossy();
            name.strip_suffix(".git").unwrap_or(&name).to_owned()
        })
        .unwrap_or_else(|| common_dir.to_owned())
}

impl HerdrWindow {
    /// Whether the bar lists Changes and the section holds anything to read.
    fn changes_wanted(&self) -> bool {
        self.activity_bar_shown() && self.activity_panel_listed(ActivityPanel::Changes)
    }

    /// Every workspace of this machine's own daemon that sits in a checkout
    /// Git may read, whichever host the window shows. Empty when this
    /// machine's daemon is not connected or not the user's own.
    fn change_targets(&self) -> Vec<(Input, Workspace)> {
        let Some(endpoint) = self.endpoints.first() else {
            return Vec::new();
        };
        let live = if self.selected_endpoint == 0 {
            &self.live
        } else {
            &endpoint.live
        };
        if !endpoint.enabled || !live.local_daemon_peer || !live.status.is_connected() {
            return Vec::new();
        }
        live.snapshot
            .iter()
            .flat_map(|snapshot| &snapshot.workspaces)
            .filter_map(|workspace| {
                let input = workspace_input(workspace, &Origin::Local).ok()?;
                Some((
                    input,
                    Workspace {
                        id: workspace.workspace_id.clone(),
                        label: workspace.label.clone(),
                    },
                ))
            })
            .take(MAX_CHECKOUTS)
            .collect()
    }

    /// Runs every tick: starts a read when one is due, or at once when the
    /// workspaces changed since the last one.
    pub(crate) fn poll_changes(&mut self, cx: &mut Context<Self>) {
        if !self.changes_wanted() {
            return;
        }
        let targets = self.change_targets();
        let inputs: Vec<Input> = targets.iter().map(|(input, _)| input.clone()).collect();
        if inputs != self.changes.inputs {
            self.changes.inputs = inputs;
            self.changes.refresh();
        }
        let now = Instant::now();
        let interval = if self.activity_section() == Some(ActivityPanel::Changes) {
            SHOWN
        } else {
            HIDDEN
        };
        // Opening the section keeps no one waiting on the slower schedule
        // the bar's count alone reads on.
        if self.changes.next.is_some_and(|next| next > now + interval) {
            self.changes.next = Some(now);
        }
        if !self.changes.due(now) {
            return;
        }
        let cancelled = self.changes.cancelled.clone();
        let work = cx.background_executor().spawn(async move {
            let stop = || cancelled.load(Ordering::Relaxed);
            let reads = targets
                .into_iter()
                .take_while(|_| !stop())
                .map(|(input, workspace)| {
                    let result = changes::read(&input, Instant::now() + TIMEOUT, &stop);
                    (workspace, result)
                })
                .collect();
            group(reads)
        });
        self.changes.task = Some(cx.spawn(async move |this, cx| {
            let read = work.await;
            let _ = this.update(cx, |this, cx| {
                this.changes
                    .install(read, Instant::now() + interval, SystemTime::now());
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Shows workspace `id` of this machine with its terminals.
    fn open_changes_workspace(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.go_to_workspace(0, id, cx);
        self.show_activity_panel(ActivityPanel::Workspaces, window, cx);
    }
}
