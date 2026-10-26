//! The activity bar's Changes section: what every checkout on this machine
//! holds in Git right now. Each checkout lists its uncommitted files with
//! their line counts, how far its branch is from its upstream, its commits
//! not yet on the remote's default branch, and its latest commits, grouped
//! by repository so a repository's worktrees sit together. What needs the
//! user leads: dirty checkouts, then unpushed, then unmerged, then behind;
//! clean ones are folded away until All is picked. What is picked in the
//! list, a commit or a checkout's uncommitted changes, is reviewed beside it
//! (`pane`).
//!
//! The checkouts come from this machine's own daemon, the trust boundary
//! the titlebar's Git chip keeps: every workspace's folder and every pane's
//! working directory, then every other worktree of the repositories those
//! are in, so a worktree an agent left behind still shows after its pane
//! closed. `[changes]` adds checkouts and folders of repositories from the
//! config (`discover`). A background pass runs every [`SHOWN`] while the
//! section is on screen and every [`HIDDEN`] otherwise, so the bar's count
//! stays current; drawing reads what it left.

mod discover;
mod files;
mod noise;
mod pane;
mod section;

#[cfg(test)]
mod tests;

use crate::{HerdrWindow, sidebar::ActivityPanel};
pub(crate) use discover::{Targets, discover};
use gpui::{prelude::*, *};
use herdr_vcs::changes::Checkout;
pub(crate) use section::age;
use std::{
    collections::HashSet,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

/// How often to read again while the section is shown.
const SHOWN: Duration = Duration::from_secs(15);
/// How often to read again for the bar's count alone.
const HIDDEN: Duration = Duration::from_secs(120);

/// A workspace with a pane or its folder in a checkout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Workspace {
    pub(crate) id: String,
    pub(crate) label: String,
}

/// A checkout as last read, and the workspaces open in it, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Shown {
    pub(crate) checkout: Checkout,
    pub(crate) workspaces: Vec<Workspace>,
}

impl Shown {
    pub(crate) fn rank(&self) -> Rank {
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
    Unmerged,
    Behind,
    Clean,
}

pub(crate) fn rank(checkout: &Checkout) -> Rank {
    if checkout.dirty() {
        Rank::Dirty
    } else if checkout.ahead > 0 {
        Rank::Unpushed
    } else if checkout.unmerged_count() > 0 {
        Rank::Unmerged
    } else if checkout.behind > 0 {
        Rank::Behind
    } else {
        Rank::Clean
    }
}

/// A checkout or folder the config names that could not be read, and why.
#[derive(Debug)]
pub(crate) struct Failed {
    pub(crate) label: String,
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
    /// What the last read was asked for, so a workspace opened, a pane
    /// moved to another folder, or the config changed reads again at once.
    targets: Targets,
    /// Whether clean checkouts are listed too.
    pub(crate) all: bool,
    /// Tells a read in flight to stop when the window goes.
    cancelled: Arc<AtomicBool>,
    /// Checkouts whose generated and binary files are listed one by one.
    noise_shown: HashSet<String>,
    /// What the review beside the list shows, and the handle the list
    /// holds the keyboard with once clicked.
    picked: Option<pane::Pick>,
    focus: Option<FocusHandle>,
    scroll: ScrollHandle,
}

impl Default for Changes {
    fn default() -> Self {
        Self {
            read: Read::default(),
            read_at: None,
            next: None,
            task: None,
            targets: Targets::default(),
            all: false,
            cancelled: Arc::default(),
            noise_shown: HashSet::new(),
            picked: None,
            focus: None,
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

    /// Checkouts that ask nothing of the user, folded away unless All is
    /// picked.
    pub(crate) fn clean(&self) -> usize {
        self.read
            .repos
            .iter()
            .flat_map(|repo| &repo.checkouts)
            .filter(|shown| shown.rank() == Rank::Clean)
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

/// Groups checkouts by repository and orders repositories and their
/// checkouts by what they ask of the user, then by name and path.
pub(crate) fn group(shown: Vec<Shown>) -> Vec<Repo> {
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
    repos
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
    /// Whether the bar lists Changes, so there is a count to keep, or the
    /// Folders page shows what the reads find.
    fn changes_wanted(&self) -> bool {
        self.activity_bar_shown()
            && (self.activity_panel_listed(ActivityPanel::Changes)
                || self.activity_section() == Some(ActivityPanel::Folders))
    }

    /// Where this pass looks: every workspace folder and pane directory of
    /// this machine's own daemon, whichever host the window shows, and what
    /// `[changes]` names. Nothing of the daemon when it is not connected or
    /// not the user's own.
    fn change_targets(&self) -> Targets {
        let mut targets = Targets {
            repos: self.config.changes.repo_paths(),
            roots: self.config.changes.root_paths(),
            ..Targets::default()
        };
        let Some(snapshot) = self.local_snapshot() else {
            return targets;
        };
        for workspace in &snapshot.workspaces {
            let shown = Workspace {
                id: workspace.workspace_id.clone(),
                label: workspace.label.clone(),
            };
            let panes = snapshot
                .panes
                .iter()
                .filter(|pane| pane.workspace_id == workspace.workspace_id)
                .flat_map(|pane| [pane.cwd.as_deref(), pane.foreground_cwd.as_deref()])
                .flatten();
            for directory in std::iter::once(workspace.new_workspace_cwd.as_str()).chain(panes) {
                targets.add_open(directory, &shown);
            }
        }
        targets
    }

    /// This machine's own daemon's state, when it is connected and the
    /// user's own, whichever host the window shows.
    pub(crate) fn local_snapshot(&self) -> Option<&herdr_client::protocol::ClientShellSnapshot> {
        let endpoint = self.endpoints.first()?;
        let live = if self.selected_endpoint == 0 {
            &self.live
        } else {
            &endpoint.live
        };
        if !endpoint.enabled || !live.local_daemon_peer || !live.status.is_connected() {
            return None;
        }
        live.snapshot.as_deref()
    }

    /// Runs every tick: starts a pass when one is due, or at once when what
    /// it would look at changed since the last one.
    pub(crate) fn poll_changes(&mut self, cx: &mut Context<Self>) {
        if !self.changes_wanted() {
            return;
        }
        let targets = self.change_targets();
        if targets != self.changes.targets {
            self.changes.targets = targets.clone();
            self.changes.refresh();
        }
        let now = Instant::now();
        let interval = if matches!(
            self.activity_section(),
            Some(ActivityPanel::Changes | ActivityPanel::Folders)
        ) {
            SHOWN
        } else {
            HIDDEN
        };
        // Opening the section keeps no one waiting on the slower schedule
        // the bar's count alone reads on.
        if self.changes.next.is_some_and(|next| next > now + interval) {
            self.changes.next = Some(now);
        }
        self.pick_first_change(cx);
        if !self.changes.due(now) {
            return;
        }
        let cancelled = self.changes.cancelled.clone();
        let work = cx
            .background_executor()
            .spawn(async move { discover(&targets, &|| cancelled.load(Ordering::Relaxed)) });
        self.changes.task = Some(cx.spawn(async move |this, cx| {
            let read = work.await;
            let _ = this.update(cx, |this, cx| {
                this.changes
                    .install(read, Instant::now() + interval, SystemTime::now());
                this.pick_first_change(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Shows or hides a checkout's generated and binary files one by one.
    pub(crate) fn toggle_changes_noise(&mut self, checkout: String, cx: &mut Context<Self>) {
        if !self.changes.noise_shown.remove(&checkout) {
            self.changes.noise_shown.insert(checkout);
        }
        cx.notify();
    }

    /// Shows workspace `id` of this machine with its terminals.
    fn open_changes_workspace(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.go_to_workspace(0, id, cx);
        self.show_activity_panel(ActivityPanel::Workspaces, window, cx);
    }
}
