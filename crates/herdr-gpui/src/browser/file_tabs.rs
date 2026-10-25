//! File tabs belong to the Files view. A code tab, or a markdown page the
//! user opened rather than an agent, is listed only while the activity bar
//! shows Files; every other view keeps them out of its strips, as an editor
//! keeps its open files apart from its terminals. Leaving Files turns each
//! group off the file it showed, and coming back shows it again. Pages an
//! agent opened stay in every view: they are the agent's hand-off, not the
//! user's browsing. Only this window's strips change; no tab closes.

use super::{Location, Pick, Store, Tab, TabId, layouts::Key};
use crate::HerdrWindow;
use gpui::Context;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub(crate) struct FileTabs {
    /// The file tabs the store has, as last counted.
    ids: HashSet<TabId>,
    /// What each workspace's groups showed when Files was left.
    aside: HashMap<Key, Vec<(super::GroupId, Pick)>>,
}

impl FileTabs {
    pub(crate) fn holds(&self, pick: &Pick) -> bool {
        matches!(pick, Pick::Page(id) if self.ids.contains(id))
    }
}

/// Whether `tab` is a file the user opened to look at.
pub(crate) fn is_file_tab(tab: &Tab) -> bool {
    tab.origin.is_none()
        && match &tab.location {
            Some(Location::Code { .. }) => true,
            Some(Location::Local { file }) => super::markdown_toggle::renders(&file.path()),
            _ => false,
        }
}

impl HerdrWindow {
    /// Runs every tick: counts the store's file tabs again, redrawing the
    /// strips when that changed.
    pub(crate) fn sync_file_tabs(&mut self, cx: &mut Context<Self>) {
        let Some(store) = cx.try_global::<Store>() else {
            return;
        };
        let ids: HashSet<TabId> = store
            .tabs()
            .filter(|tab| is_file_tab(tab))
            .map(|tab| tab.id)
            .collect();
        if ids != self.browser.file_tabs.ids {
            self.browser.file_tabs.ids = ids;
            cx.notify();
        }
    }

    /// Whether this window's strips leave out file tabs now.
    pub(crate) fn sets_files_aside(&self, pick: &Pick) -> bool {
        !self.files_view() && self.browser.file_tabs.holds(pick)
    }

    /// Turns the groups off their file tabs on leaving Files, or back to
    /// them on coming back.
    pub(crate) fn files_view_changed(&mut self, cx: &mut Context<Self>) {
        self.sync_file_tabs(cx);
        let shown = self.files_view();
        let browser = &mut self.browser;
        if shown {
            let aside = std::mem::take(&mut browser.file_tabs.aside);
            let store = cx.try_global::<Store>();
            for (key, taken) in aside {
                let open: Vec<_> = taken
                    .into_iter()
                    .filter(|(_, pick)| match pick {
                        Pick::Page(id) => store.is_some_and(|store| store.get(*id).is_some()),
                        Pick::Herdr(_) => false,
                    })
                    .collect();
                if let Some(layout) = browser.layouts.get_mut(&key) {
                    layout.bring_back(open);
                }
            }
        } else {
            let files = &browser.file_tabs;
            let mut aside = HashMap::new();
            for (key, layout) in &mut browser.layouts {
                let taken = layout.set_aside(|pick| files.holds(pick));
                if !taken.is_empty() {
                    aside.insert(key.clone(), taken);
                }
            }
            browser.file_tabs.aside = aside;
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
