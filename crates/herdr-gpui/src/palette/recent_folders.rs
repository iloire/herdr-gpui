//! Local folders this app has seen open as workspaces, so the + on the
//! Folders list can offer one again after its workspace was closed. Observed
//! from the local daemon's snapshots, newest first, kept per user on this
//! machine and never sent to the daemon.

use super::{Action, Entry, canonical, projects};
use crate::{HerdrWindow, state_file};
use gpui::{App, Global};
use herdr_client::protocol::ClientShellSnapshot;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Weak},
};

/// Folders remembered; the longest unused go first beyond this.
const MAX_FOLDERS: usize = 64;
/// Bytes in a remembered path. A longer one is not remembered, so every
/// folder the store accepts fits in the file it reads back.
const MAX_PATH_BYTES: usize = 4096;
/// JSON escapes at most double a string's bytes, plus the field names.
const MAX_FILE_BYTES: u64 = (MAX_FOLDERS * (2 * MAX_PATH_BYTES + 64) + 64) as u64;
const FILE: &str = "recent-folders.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Folder {
    path: PathBuf,
    /// Unix seconds when the folder was last seen opening or closing.
    at: u64,
}

fn rememberable(path: &Path) -> bool {
    path.is_absolute()
        && path
            .to_str()
            .is_some_and(|text| text.len() <= MAX_PATH_BYTES && !text.chars().any(char::is_control))
}

#[derive(Clone, Serialize, Deserialize)]
struct Saved {
    folders: Vec<Folder>,
}

fn parse(bytes: &[u8]) -> crate::Result<Vec<Folder>> {
    let saved: Saved = serde_json::from_slice(bytes)?;
    if saved.folders.len() > MAX_FOLDERS || !saved.folders.iter().all(|f| rememberable(&f.path)) {
        return Err(super::Error::InvalidRecentFolders.into());
    }
    Ok(saved.folders)
}

/// The app's recent folders.
#[derive(Default)]
pub(crate) struct RecentFolders {
    /// Newest first.
    folders: Vec<Folder>,
    /// The folders open in the snapshot last observed.
    open: Vec<PathBuf>,
    /// That snapshot, compared by address only so an unchanged one costs
    /// nothing and a dropped one is not kept alive.
    observed: Weak<ClientShellSnapshot>,
    writer: Option<state_file::Writer<Saved>>,
    quitting: bool,
}

impl Global for RecentFolders {}

impl RecentFolders {
    /// Called before starting GPUI, like the palette history. A missing or
    /// damaged file starts with no folders.
    pub(crate) fn load() -> Self {
        Self::at(crate::preferences::state_dir().map(|dir| dir.join(FILE)))
    }

    fn at(path: Option<PathBuf>) -> Self {
        let folders = path
            .as_deref()
            .map(|path| {
                state_file::read(path, MAX_FILE_BYTES)
                    .and_then(|bytes| bytes.as_deref().map_or(Ok(Vec::new()), parse))
            })
            .transpose()
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "Cannot restore recent folders");
                None
            })
            .unwrap_or_default();
        let writer = path.and_then(|path| {
            state_file::Writer::start("recent-folders", path)
                .inspect_err(
                    |error| tracing::warn!(%error, "Cannot start the recent folders worker"),
                )
                .ok()
        });
        Self {
            folders,
            writer,
            ..Self::default()
        }
    }

    pub(crate) fn install(self, cx: &mut App) {
        cx.set_global(self);
        cx.on_app_quit(|cx| {
            let recent = cx.global_mut::<Self>();
            recent.quitting = true;
            let writer = recent.writer.take();
            cx.background_executor().spawn(async move {
                if let Some(writer) = writer {
                    writer.finish();
                }
            })
        })
        .detach();
    }

    /// Whether any folder is remembered.
    pub(super) fn any(cx: &App) -> bool {
        cx.try_global::<Self>()
            .is_some_and(|recent| !recent.folders.is_empty())
    }

    /// The remembered folders, newest first.
    pub(super) fn paths(cx: &App) -> Vec<PathBuf> {
        cx.try_global::<Self>()
            .map(|recent| recent.folders.iter().map(|f| f.path.clone()).collect())
            .unwrap_or_default()
    }

    /// Notes the folders open in `snapshot` at `now`. A folder that opens or
    /// closes moves to the front, so the list reads as last used; a snapshot
    /// that changes nothing else, such as an agent's status, writes nothing.
    pub(super) fn observe(&mut self, snapshot: &Arc<ClientShellSnapshot>, now: u64) {
        if Weak::ptr_eq(&self.observed, &Arc::downgrade(snapshot)) {
            return;
        }
        self.observed = Arc::downgrade(snapshot);
        let open: Vec<PathBuf> = projects::workspace_roots(snapshot)
            .into_iter()
            .map(|(_, root)| PathBuf::from(root))
            .filter(|path| rememberable(path))
            .collect();
        if open == self.open {
            return;
        }
        let changed: Vec<PathBuf> = open
            .iter()
            .filter(|path| !self.open.contains(path))
            .chain(self.open.iter().filter(|path| !open.contains(path)))
            .cloned()
            .collect();
        self.open = open;
        if changed.is_empty() {
            return;
        }
        self.folders
            .retain(|folder| !changed.contains(&folder.path));
        self.folders.splice(
            0..0,
            changed.into_iter().map(|path| Folder { path, at: now }),
        );
        self.folders.truncate(MAX_FOLDERS);
        if !self.quitting
            && let Some(writer) = &self.writer
        {
            writer.save(Saved {
                folders: self.folders.clone(),
            });
        }
    }
}

impl HerdrWindow {
    /// Remembers the folders open on this machine's own daemon. Remote hosts
    /// are not remembered: their folders cannot be opened from here.
    pub(crate) fn remember_open_folders(&self, cx: &mut App) {
        let Some(snapshot) = self.local_snapshot() else {
            return;
        };
        if !cx.has_global::<RecentFolders>() {
            cx.set_global(RecentFolders::default());
        }
        cx.global_mut::<RecentFolders>()
            .observe(snapshot, super::frecency::now());
    }
}

/// The row that opens the system's folder dialog.
pub(super) fn choose_entry() -> Entry {
    Entry::with_keywords(
        "Choose a folder…".into(),
        "Browse this computer for any folder to add".into(),
        "Browse",
        Action::ChooseFolder,
        None,
        "open add directory finder",
    )
}

/// The remembered folders that still exist and are not open now, as projects
/// to open. Blocking: reads the disk, so callers run it off the UI thread.
pub(super) fn closed(recent: Vec<PathBuf>, open: &[String]) -> Vec<projects::Project> {
    let open: Vec<PathBuf> = open
        .iter()
        .filter_map(|root| canonical::canonicalize(Path::new(root)).ok())
        .collect();
    let mut seen = Vec::new();
    recent
        .into_iter()
        .filter_map(|path| {
            let path = canonical::canonicalize(&path).ok()?;
            if !path.is_dir() || open.contains(&path) || seen.contains(&path) {
                return None;
            }
            seen.push(path.clone());
            let label = path.file_name()?.to_string_lossy().into_owned();
            Some(projects::Project { path, label })
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
