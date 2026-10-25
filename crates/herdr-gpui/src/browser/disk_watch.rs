//! Tabs showing a local file follow it on disk: a code tab reads its file
//! again, and a rendered markdown page reloads, once the file's modified
//! time moves, so a plan an agent is writing shows its latest save. Only
//! the focused workspace's tabs are watched, a bounded number of them, and
//! the times are read on the background executor about once a second.

use super::{Location, Store, TabId};
use crate::HerdrWindow;
use gpui::{Context, Task};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};

/// How often the watched files' times are read.
const EVERY: Duration = Duration::from_secs(1);
/// The most tabs watched at once.
const WATCHED: usize = 32;

/// What a watched tab does when its file changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Follow {
    Read,
    Reload,
}

#[derive(Default)]
pub(crate) struct DiskWatch {
    /// Each watched tab's file time as last read.
    stamps: HashMap<TabId, SystemTime>,
    next: Option<Instant>,
    task: Option<Task<()>>,
}

impl DiskWatch {
    /// Records `read`, the times just read, and names the tabs whose file
    /// moved since the read before. A tab seen for the first time only
    /// records. A file that cannot be read keeps its last time: a save that
    /// writes a new file and renames it over the old one leaves a moment
    /// with no file, and the save after it must still count as a change.
    fn settle(&mut self, read: Vec<(TabId, Follow, Option<SystemTime>)>) -> Vec<(TabId, Follow)> {
        let mut changed = Vec::new();
        let mut stamps = HashMap::with_capacity(read.len());
        for (id, follow, stamp) in read {
            let before = self.stamps.get(&id).copied();
            let Some(stamp) = stamp.or(before) else {
                continue;
            };
            if before.is_some_and(|before| before != stamp) {
                changed.push((id, follow));
            }
            stamps.insert(id, stamp);
        }
        self.stamps = stamps;
        changed
    }
}

/// The file tab `location` follows, and how.
fn watched(location: &Location) -> Option<(PathBuf, Follow)> {
    match location {
        Location::Code { file } => Some((file.path.clone().into(), Follow::Read)),
        Location::Local { file } => {
            let path = file.path();
            super::markdown_toggle::renders(&path).then_some((path, Follow::Reload))
        }
        _ => None,
    }
}

impl HerdrWindow {
    /// Runs every tick: reads the watched files' times when due, then
    /// refreshes the tabs whose file changed.
    pub(crate) fn poll_tab_files(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let disk = &self.browser.disk;
        if disk.task.is_some() || disk.next.is_some_and(|next| now < next) {
            return;
        }
        let Some((scope, workspace)) = self.browser_key() else {
            return;
        };
        let Some(store) = cx.try_global::<Store>() else {
            return;
        };
        let files: Vec<(TabId, PathBuf, Follow)> = store
            .in_workspace(&scope, &workspace)
            .filter_map(|tab| {
                let (path, follow) = watched(tab.location.as_ref()?)?;
                Some((tab.id, path, follow))
            })
            .take(WATCHED)
            .collect();
        if files.is_empty() {
            self.browser.disk = DiskWatch::default();
            return;
        }
        let read = cx.background_executor().spawn(async move {
            files
                .into_iter()
                .map(|(id, path, follow)| {
                    let stamp = std::fs::metadata(path)
                        .and_then(|meta| meta.modified())
                        .ok();
                    (id, follow, stamp)
                })
                .collect::<Vec<_>>()
        });
        self.browser.disk.task = Some(cx.spawn(async move |this, cx| {
            let read = read.await;
            let _ = this.update(cx, |this, cx| {
                let disk = &mut this.browser.disk;
                disk.task = None;
                disk.next = Some(Instant::now() + EVERY);
                for (id, follow) in disk.settle(read) {
                    match follow {
                        Follow::Read => this.load_code_view(id, cx),
                        #[cfg(any(target_os = "macos", windows))]
                        Follow::Reload => this.browser.pages.reload(id),
                        #[cfg(not(any(target_os = "macos", windows)))]
                        Follow::Reload => {}
                    }
                }
            });
        }));
    }
}

#[cfg(test)]
mod tests;
