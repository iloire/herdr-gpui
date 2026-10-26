//! The activity bar's Folders section: every open workspace on every host
//! this window connects to, as a folder in a list on the left, what needs
//! the user first. The selected folder unfolds into what it holds, and the
//! part picked fills the rest of the window: Home, its branch, uncommitted
//! files, agents, commits, and ports together; Changes, its uncommitted
//! files and commits with the review beside them; Terminals, its
//! workspace's terminals and tabs; or Files, its file tree beside them.
//!
//! Home and Changes are pages across the window's body. Terminals and Files
//! need the window's own terminal area, so with either picked the list
//! becomes a column beside it, as the Workspaces sidebar is (`column`),
//! and the folder shown is the workspace the window shows. Each folder
//! keeps the part it was left on.
//!
//! Everything it shows is already in the window: the endpoints' snapshots
//! (whose branch and ahead/behind each daemon reads on its own host), the
//! Changes reader's checkouts on this machine, the Overview's agent rows,
//! and the port monitors. Drawing reads that prepared state only.

mod column;
mod home;
mod list;
mod model;
mod view;

#[cfg(test)]
mod tests;

use crate::{HerdrWindow, panel_resize::PanelWidth, sidebar::ActivityPanel};
use gpui::{prelude::*, *};
use model::{Card, Host};
use std::collections::HashMap;

/// What the window shows of the selected folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Part {
    #[default]
    Home,
    Terminals,
    Changes,
    Files,
}

impl Part {
    /// Whether the part is the window's terminal area, with the folder list
    /// as a column beside it, rather than a page across the body.
    pub(crate) fn beside_terminals(self) -> bool {
        matches!(self, Self::Terminals | Self::Files)
    }
}

/// A folder, by endpoint and workspace id.
type Key = (String, String);

/// The page's own state in a window.
pub(crate) struct Page {
    /// Whether every folder is listed, or only those that need the user.
    all: bool,
    /// The folder unfolded; the first listed when unset or gone.
    selected: Option<Key>,
    /// The selected folder's part.
    pub(crate) part: Part,
    /// The part each folder was last left on.
    parts: HashMap<Key, Part>,
    /// The list's width, as dragged.
    pub(crate) list_width: PanelWidth,
    list_scroll: ScrollHandle,
    body_scroll: ScrollHandle,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            all: true,
            selected: None,
            part: Part::Home,
            parts: HashMap::new(),
            list_width: crate::panel_resize::FOLDERS_LIST,
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

    /// The folder unfolded: beside the terminals, the workspace the window
    /// shows, which the keyboard may have moved to; else the one picked, else
    /// the first listed.
    fn selected_folder<'a>(&self, listed: &[&'a Card]) -> Option<&'a Card> {
        let shown = || {
            listed
                .iter()
                .find(|card| self.shows_workspace(card.endpoint_index, &card.workspace))
        };
        let picked = || {
            self.folders
                .selected
                .as_ref()
                .and_then(|(endpoint, workspace)| {
                    listed
                        .iter()
                        .find(|card| card.endpoint == *endpoint && card.workspace == *workspace)
                })
        };
        let shown = self.folders.part.beside_terminals().then(shown).flatten();
        shown.or_else(picked).or_else(|| listed.first()).copied()
    }

    /// Unfolds a folder at `part`, or, with none given, at the part it was
    /// left on, else the part shown now. Terminals and Files show its
    /// workspace with the keyboard in its terminal.
    fn select_folder(
        &mut self,
        card: &Card,
        part: Option<Part>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = (card.endpoint.clone(), card.workspace.clone());
        let part = part
            .or_else(|| self.folders.parts.get(&key).copied())
            .unwrap_or(self.folders.part);
        let files = self.files_view();
        self.folders.selected = Some(key.clone());
        self.folders.part = part;
        self.folders.parts.insert(key, part);
        if files != self.files_view() {
            self.files_view_changed(cx);
        }
        self.sync_activity_section(cx);
        match part {
            Part::Home => self.focus_folders_page(window, cx),
            Part::Changes => {
                if let Some(checkout) = &card.checkout {
                    self.pick_checkout_change(checkout, cx);
                }
                self.focus_folders_page(window, cx);
            }
            Part::Terminals | Part::Files => {
                self.go_to_workspace(card.endpoint_index, &card.workspace, cx);
                window.focus(&self.focus, cx);
            }
        }
        cx.notify();
    }

    /// Puts the keyboard on the Folders page, as picking the section does.
    fn focus_folders_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self
            .activity
            .section_focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        window.focus(&focus, cx);
    }

    /// Opens `card`'s agent pane in its Terminals, staying in Folders.
    fn open_folder_agent(
        &mut self,
        card: &Card,
        row: &crate::overview::Row,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_overview_row(row, window, cx);
        self.show_activity_panel(ActivityPanel::Folders, window, cx);
        self.select_folder(card, Some(Part::Terminals), window, cx);
    }

    fn show_all_folders(&mut self, all: bool, cx: &mut Context<Self>) {
        self.folders.all = all;
        cx.notify();
    }
}
