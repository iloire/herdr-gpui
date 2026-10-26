//! The activity bar's Folders section: every open workspace on every host
//! this window connects to, as a folder in a list on the left, what needs
//! the user first. The selected folder unfolds into what it holds, and the
//! part picked fills the body: Home, its branch, uncommitted files, agents,
//! commits, and ports together; Changes, its uncommitted files and commits
//! with the review beside them; or Terminals and Files, which open the
//! workspace with its terminals or its file tree.
//!
//! Everything it shows is already in the window: the endpoints' snapshots
//! (whose branch and ahead/behind each daemon reads on its own host), the
//! Changes reader's checkouts on this machine, the Overview's agent rows,
//! and the port monitors. Drawing reads that prepared state only.

mod home;
mod list;
mod model;
mod view;

#[cfg(test)]
mod tests;

use crate::{HerdrWindow, sidebar::ActivityPanel};
use gpui::{prelude::*, *};
use model::{Card, Host};

/// What the body shows of the selected folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Part {
    #[default]
    Home,
    Changes,
}

/// The page's own state in a window.
pub(crate) struct Page {
    /// Whether every folder is listed, or only those that need the user.
    all: bool,
    /// The folder unfolded, by endpoint and workspace id; the first listed
    /// when unset or gone.
    selected: Option<(String, String)>,
    part: Part,
    list_scroll: ScrollHandle,
    body_scroll: ScrollHandle,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            all: true,
            selected: None,
            part: Part::Home,
            list_scroll: ScrollHandle::new(),
            body_scroll: ScrollHandle::new(),
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

    /// The folders the list shows, as the filter leaves them.
    fn listed_folders<'a>(&self, cards: &'a [Card]) -> Vec<&'a Card> {
        cards
            .iter()
            .filter(|card| self.folders.all || card.needs_me())
            .collect()
    }

    /// The folder unfolded: the one picked, else the first listed.
    fn selected_folder<'a>(&self, listed: &[&'a Card]) -> Option<&'a Card> {
        let picked = self
            .folders
            .selected
            .as_ref()
            .and_then(|(endpoint, workspace)| {
                listed
                    .iter()
                    .find(|card| card.endpoint == *endpoint && card.workspace == *workspace)
            });
        picked.or_else(|| listed.first()).copied()
    }

    /// Unfolds a folder, or shows another part of the one unfolded.
    fn select_folder(&mut self, card: &Card, part: Part, cx: &mut Context<Self>) {
        self.folders.selected = Some((card.endpoint.clone(), card.workspace.clone()));
        self.folders.part = part;
        if part == Part::Changes
            && let Some(checkout) = &card.checkout
        {
            self.pick_checkout_change(checkout, cx);
        }
        cx.notify();
    }

    /// Shows a folder's workspace in `panel`: with its terminals, or with
    /// its file tree beside them.
    fn open_folder(
        &mut self,
        index: usize,
        workspace: &str,
        panel: ActivityPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.go_to_workspace(index, workspace, cx);
        self.show_activity_panel(panel, window, cx);
        self.activity.return_to = Some(ActivityPanel::Folders);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn show_all_folders(&mut self, all: bool, cx: &mut Context<Self>) {
        self.folders.all = all;
        cx.notify();
    }
}
