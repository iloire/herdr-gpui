//! The activity bar's Folders section: every open workspace on every host
//! this window connects to, as a card saying what needs the user there.
//! A card shows the folder's branch and how far it is from its upstream,
//! its uncommitted files, its most pressing agent, and the ports it listens
//! on; clicking it opens that workspace with its terminals. Cards waiting
//! on the user lead, then dirty, unpushed, and working ones.
//!
//! Everything it shows is already in the window: the endpoints' snapshots
//! (whose branch and ahead/behind each daemon reads on its own host), the
//! Changes reader's uncommitted files of this machine's checkouts, the
//! Overview's agent rows, and the port monitors. Drawing reads that
//! prepared state only.

mod model;
mod view;

#[cfg(test)]
mod tests;

use crate::{HerdrWindow, sidebar::ActivityPanel};
use gpui::{prelude::*, *};
use model::{Card, Host};

/// The page's own state in a window.
pub(crate) struct Page {
    /// Whether every card is listed, or only those that need the user.
    all: bool,
    scroll: ScrollHandle,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            all: true,
            scroll: ScrollHandle::new(),
        }
    }
}

impl HerdrWindow {
    /// Every connected host's workspaces as cards, most pressing first.
    fn folder_cards(&self, cx: &App) -> Vec<Card> {
        let rows = self.overview_rows(cx);
        let local = self.local_snapshot().is_some();
        let multi = self.endpoints.len() > 1;
        let hosts = self
            .endpoints
            .iter()
            .enumerate()
            .filter_map(|(index, endpoint)| {
                let live = if index == self.selected_endpoint {
                    &self.live
                } else {
                    &endpoint.live
                };
                let snapshot = live
                    .snapshot
                    .as_deref()
                    .filter(|_| endpoint.enabled && live.status.is_connected())?;
                Some(Host {
                    index,
                    id: &endpoint.id,
                    label: (multi && endpoint.id != crate::endpoint::LOCAL)
                        .then_some(endpoint.label.as_str()),
                    snapshot,
                    // The Changes reader reads this machine's own daemon only.
                    repos: (index == 0 && local).then(|| self.changes.repos()),
                })
            });
        model::cards(hosts, &rows)
    }

    /// Shows a card's workspace with its terminals.
    fn open_folder(
        &mut self,
        index: usize,
        workspace: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.go_to_workspace(index, workspace, cx);
        self.show_activity_panel(ActivityPanel::Workspaces, window, cx);
        self.activity.return_to = Some(ActivityPanel::Folders);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn show_all_folders(&mut self, all: bool, cx: &mut Context<Self>) {
        self.folders.all = all;
        cx.notify();
    }
}
