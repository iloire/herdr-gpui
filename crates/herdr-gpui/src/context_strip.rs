//! The context strip: one line between the terminal and the status bar about
//! the focused pane's checkout. Its branch and how far it is from its
//! upstream, its uncommitted files and lines, stashes, the repository's
//! other worktrees with work not merged, the branch's pull request, and
//! what Git status leaves out: a rebase or merge stopped halfway, or a
//! git-crypt repository still locked.
//!
//! Only this machine's own daemon is read, the trust boundary the Changes
//! section keeps: on another host the strip names the folder and no more.
//! The focused pane's folder is read on the background executor every
//! [`INTERVAL`], and at once when focus moves to another folder; drawing
//! reads what the last read left. The strip keeps its row whatever the
//! folder holds, so moving focus never resizes the terminal.

mod render;

#[cfg(test)]
mod tests;

use crate::HerdrWindow;
use gpui::{prelude::*, *};
use herdr_vcs::context;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// How often the focused folder is read again.
const INTERVAL: Duration = Duration::from_secs(10);
/// One read's Git children together, as the Changes section allows.
const TIMEOUT: Duration = Duration::from_secs(15);

/// The folder the strip describes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) directory: String,
    /// On this machine's own daemon, so its checkout can be read here.
    pub(crate) local: bool,
    /// The focused workspace's repository key, under which its pull
    /// requests are cached.
    pub(crate) repo_key: Option<String>,
}

/// The strip's state in a window.
#[derive(Default)]
pub(crate) struct ContextStrip {
    target: Option<Target>,
    /// The last read of the target's folder, `None` until one finishes.
    read: Option<herdr_vcs::Result<context::Context>>,
    /// When the next read is due; `None` reads at the next tick.
    next: Option<Instant>,
    task: Option<Task<()>>,
    /// Tells a read in flight to stop when the window goes.
    cancelled: Arc<AtomicBool>,
}

impl Drop for ContextStrip {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl ContextStrip {
    /// Points the strip at `target`, forgetting a read of another folder so
    /// it never describes the wrong one.
    fn aim(&mut self, target: Option<Target>) -> bool {
        if target == self.target {
            return false;
        }
        let directory = |target: &Option<Target>| target.as_ref().map(|t| t.directory.clone());
        if directory(&target) != directory(&self.target) {
            self.read = None;
            self.next = None;
        }
        self.target = target;
        true
    }

    fn due(&self, now: Instant) -> bool {
        self.task.is_none() && self.next.is_none_or(|next| now >= next)
    }

    /// Keeps a finished read of `directory` if focus is still there, and
    /// otherwise reads the new folder at the next tick.
    fn install(
        &mut self,
        directory: &str,
        read: herdr_vcs::Result<context::Context>,
        next: Instant,
    ) {
        self.task = None;
        if self.target.as_ref().map(|t| t.directory.as_str()) == Some(directory) {
            self.read = Some(read);
            self.next = Some(next);
        } else {
            self.next = None;
        }
    }

    /// Installs `read` for `target` as a pass would, for tests.
    #[cfg(test)]
    pub(crate) fn seed(&mut self, target: Target, read: herdr_vcs::Result<context::Context>) {
        self.aim(Some(target.clone()));
        self.install(&target.directory, read, Instant::now() + INTERVAL);
    }
}

impl HerdrWindow {
    /// The focused pane's folder on the host the window shows: what its
    /// foreground process runs in, else its shell's.
    fn context_target(&self) -> Option<Target> {
        let snapshot = self.live.snapshot.as_deref()?;
        let pane_id = snapshot.focused_pane_id.as_deref()?;
        let pane = snapshot.panes.iter().find(|pane| pane.pane_id == pane_id)?;
        let directory = [pane.foreground_cwd.as_deref(), pane.cwd.as_deref()]
            .into_iter()
            .flatten()
            .find(|directory| Path::new(directory).is_absolute())?;
        let local = self.selected_endpoint == 0 && self.local_snapshot().is_some();
        let repo_key = snapshot
            .workspaces
            .iter()
            .find(|workspace| workspace.workspace_id == pane.workspace_id)
            .and_then(|workspace| workspace.worktree.as_ref())
            .map(|worktree| worktree.key.clone());
        Some(Target {
            directory: directory.to_owned(),
            local,
            repo_key,
        })
    }

    /// Runs every tick: follows focus, and reads the focused folder when a
    /// read is due.
    pub(crate) fn poll_context_strip(&mut self, cx: &mut Context<Self>) {
        let target = self
            .config
            .show_context_strip
            .then(|| self.context_target())
            .flatten();
        if self.context_strip.aim(target) {
            cx.notify();
        }
        let Some(target) = self.context_strip.target.clone().filter(|t| t.local) else {
            return;
        };
        if !self.context_strip.due(Instant::now()) {
            return;
        }
        let directory = target.directory;
        let cancelled = self.context_strip.cancelled.clone();
        let asked = directory.clone();
        let work = cx.background_executor().spawn(async move {
            let deadline = Instant::now() + TIMEOUT;
            context::read(&asked, deadline, &|| cancelled.load(Ordering::Relaxed))
        });
        self.context_strip.task = Some(cx.spawn(async move |this, cx| {
            let read = work.await;
            let _ = this.update(cx, |this, cx| {
                this.context_strip
                    .install(&directory, read, Instant::now() + INTERVAL);
                cx.notify();
            });
        }));
    }
}
