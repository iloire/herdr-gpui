//! The activity bar's Files section: the focused pane's checkout as a tree
//! beside the terminals, as an editor's explorer sits beside its code, so
//! the user can look through what the agent works on while instructing it.
//!
//! The section follows the focused pane: when the focus moves to another
//! pane, or the agent there moves to another folder, the tree turns to that
//! folder's checkout and opens down to the folder. Pinning holds the tree
//! where it is. The files are the ones Git lists (`code_index::Index::list`),
//! so ignored build output stays out, read with the uncommitted changes on
//! the background executor every few seconds while the section is shown and
//! never while it is not. The changed files lead the tree, newest first.
//!
//! Only this computer's checkouts are read: a pane on an SSH host has its
//! files there, out of reach of a local listing.

mod render;
mod tree;

#[cfg(test)]
mod tests;

use crate::{HerdrWindow, code_index::Index};
use gpui::{prelude::*, *};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tree::{Row, Tree};

/// How often the shown tree is read again.
const SHOWN: Duration = Duration::from_secs(3);
/// The changed files listed over the tree at most.
const RECENT: usize = 8;

/// A checkout as last read.
#[derive(Debug)]
pub(crate) struct Listing {
    pub(crate) index: Arc<Index>,
    pub(crate) tree: Arc<Tree>,
}

/// What the section shows instead of a tree, or that it shows one.
#[derive(Debug, Default)]
pub(crate) enum Status {
    /// No pane to follow yet.
    #[default]
    Idle,
    Reading,
    Ready,
    /// The followed pane is on another host.
    Remote,
    Failed(crate::Error),
}

/// One read's result, for the folder it was asked about.
struct Read {
    folder: String,
    listing: crate::Result<(Listing, Option<String>)>,
}

/// The section's state in a window.
pub(crate) struct Files {
    /// The folder the tree follows, as the focused pane reported it.
    followed: Option<String>,
    /// Holds the tree on `followed` while the focus moves elsewhere.
    pub(crate) pinned: bool,
    pub(crate) listing: Option<Listing>,
    pub(crate) status: Status,
    /// The open folders of each checkout, so going back to one finds it as
    /// it was left.
    expanded: HashMap<PathBuf, HashSet<String>>,
    /// Opens the folders down to the followed one once the next read lands.
    reveal: bool,
    /// The followed folder relative to its checkout, to mark in the tree.
    pub(crate) here: Option<String>,
    pub(crate) rows: Vec<Row>,
    /// The file last opened from the tree, relative to its checkout.
    pub(crate) selected: Option<String>,
    next: Option<Instant>,
    task: Option<Task<()>>,
    /// Tells a read in flight to stop when the window goes.
    cancelled: Arc<AtomicBool>,
    pub(crate) scroll: UniformListScrollHandle,
}

impl Default for Files {
    fn default() -> Self {
        Self {
            followed: None,
            pinned: false,
            listing: None,
            status: Status::Idle,
            expanded: HashMap::new(),
            reveal: false,
            here: None,
            rows: Vec::new(),
            selected: None,
            next: None,
            task: None,
            cancelled: Arc::default(),
            scroll: UniformListScrollHandle::new(),
        }
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Files {
    /// Turns to `folder`, unless pinned; whether it moved.
    fn follow(&mut self, folder: Option<String>) -> bool {
        if self.pinned || folder == self.followed {
            return false;
        }
        self.followed = folder;
        self.reveal = true;
        self.next = None;
        true
    }

    pub(crate) fn followed(&self) -> Option<&str> {
        self.followed.as_deref()
    }

    fn due(&self, now: Instant) -> bool {
        self.task.is_none() && self.next.is_none_or(|next| now >= next)
    }

    pub(crate) fn root(&self) -> Option<&Path> {
        self.listing.as_ref().map(|listing| listing.index.root())
    }

    fn open_folders(&self) -> Option<&HashSet<String>> {
        self.expanded.get(self.root()?)
    }

    pub(crate) fn is_open(&self, folder: &str) -> bool {
        self.open_folders()
            .is_some_and(|open| open.contains(folder))
    }

    /// Opens or closes `folder` of the shown checkout.
    pub(crate) fn toggle(&mut self, folder: &str) {
        let Some(root) = self.root().map(Path::to_path_buf) else {
            return;
        };
        let open = self.expanded.entry(root).or_default();
        if !open.remove(folder) {
            open.insert(folder.to_owned());
        }
        self.lay_out();
    }

    /// Closes every folder of the shown checkout.
    pub(crate) fn collapse_all(&mut self) {
        if let Some(root) = self.root().map(Path::to_path_buf) {
            self.expanded.remove(&root);
        }
        self.lay_out();
    }

    /// Takes in a read of `followed`, opening the folders down to it when
    /// the tree just turned there, and reads again at `next`.
    fn install(&mut self, read: Read, next: Instant) {
        self.task = None;
        if self.followed.as_deref() != Some(read.folder.as_str()) {
            // The focus moved on while this read ran; the next one is due.
            return;
        }
        self.next = Some(next);
        match read.listing {
            Ok((listing, here)) => {
                let root = listing.index.root().to_path_buf();
                if std::mem::take(&mut self.reveal)
                    && let Some(here) = here.as_deref()
                {
                    let open = self.expanded.entry(root).or_default();
                    open.extend(tree::ancestors(here).map(str::to_owned));
                    open.insert(here.to_owned());
                }
                self.here = here;
                self.listing = Some(listing);
                self.status = Status::Ready;
            }
            Err(error) => {
                self.listing = None;
                self.here = None;
                self.status = Status::Failed(error);
            }
        }
        self.lay_out();
    }

    fn lay_out(&mut self) {
        self.rows = match (&self.listing, self.open_folders()) {
            (Some(listing), open) => listing.tree.rows(open.unwrap_or(&HashSet::new())),
            (None, _) => Vec::new(),
        };
    }

    /// The changed files listed over the tree, newest first.
    pub(crate) fn recent(&self) -> &[herdr_vcs::code_index::Change] {
        self.listing.as_ref().map_or(&[], |listing| {
            let changes = listing.index.changes();
            &changes[..changes.len().min(RECENT)]
        })
    }
}

/// Reads the checkout holding `folder` and where `folder` sits in it.
/// Blocking.
fn read(folder: &str, cancelled: &AtomicBool) -> crate::Result<(Listing, Option<String>)> {
    let root = crate::code_index::checkout_root(folder, cancelled)?;
    let index = Index::list(&root, cancelled)?;
    let tree = Tree::new(&index);
    let here = relative(&root, Path::new(folder));
    Ok((
        Listing {
            index: Arc::new(index),
            tree: Arc::new(tree),
        },
        here,
    ))
}

/// `folder` under `root` with `/` between its parts, or `None` at the root
/// or outside it. Either may be reached through a symlink, as `/tmp` is on
/// macOS, so both are resolved before comparing. Blocking.
fn relative(root: &Path, folder: &Path) -> Option<String> {
    let resolve = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let inner = resolve(folder);
    let inner = inner.strip_prefix(resolve(root)).ok()?;
    let parts: Vec<_> = inner
        .components()
        .map(|part| part.as_os_str().to_str())
        .collect::<Option<_>>()?;
    (!parts.is_empty()).then(|| parts.join("/"))
}

impl HerdrWindow {
    /// The focused pane's folder: where its program runs, else its shell.
    fn files_folder(&self) -> Option<String> {
        let snapshot = self.live.snapshot.as_deref()?;
        let id = snapshot.focused_pane_id.as_deref()?;
        let pane = snapshot.panes.iter().find(|pane| pane.pane_id == id)?;
        pane.foreground_cwd.clone().or_else(|| pane.cwd.clone())
    }

    /// Runs every tick: follows the focus, and reads the followed checkout
    /// when a read is due and the section is shown.
    pub(crate) fn poll_files(&mut self, cx: &mut Context<Self>) {
        if !self.files_view() {
            return;
        }
        if self.selected_is_remote() {
            if !matches!(self.files.status, Status::Remote) {
                self.files.status = Status::Remote;
                cx.notify();
            }
            return;
        }
        if self.files.follow(self.files_folder()) {
            cx.notify();
        }
        let now = Instant::now();
        if !self.files.due(now) {
            return;
        }
        let Some(folder) = self.files.followed.clone() else {
            return;
        };
        if self.files.listing.is_none() {
            self.files.status = Status::Reading;
        }
        let cancelled = self.files.cancelled.clone();
        let asked = folder.clone();
        let work = cx
            .background_executor()
            .spawn(async move { read(&asked, &cancelled) });
        self.files.task = Some(cx.spawn(async move |this, cx| {
            let listing = work.await;
            let _ = this.update(cx, |this, cx| {
                this.files
                    .install(Read { folder, listing }, Instant::now() + SHOWN);
                cx.notify();
            });
        }));
    }

    /// Holds the tree on its checkout, or lets it follow the focus again.
    pub(crate) fn toggle_files_pin(&mut self, cx: &mut Context<Self>) {
        self.files.pinned = !self.files.pinned;
        cx.notify();
    }
}
