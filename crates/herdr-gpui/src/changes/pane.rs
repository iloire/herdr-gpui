//! What the Changes section shows beside its list, as Git clients do: the
//! item picked on the left, a commit or a checkout's uncommitted changes,
//! read in the review on the right (`review::view::pane`). The list's first
//! item is picked when the section shows, and ↑ and ↓ step through it.

use super::{Rank, Shown};
use crate::{
    HerdrWindow,
    browser::ReviewCheckout,
    review::{Agent, CommitId},
    sidebar::ActivityPanel,
    window::Flash,
};
use gpui::{prelude::*, *};
use std::path::Path;

/// An item of the list that shows changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Pick {
    /// The uncommitted changes of the checkout at this path.
    Uncommitted(String),
    /// A commit, by its full name, of the checkout at `checkout`.
    Commit { checkout: String, sha: String },
}

impl Pick {
    fn checkout(&self) -> &str {
        match self {
            Self::Uncommitted(checkout) | Self::Commit { checkout, .. } => checkout,
        }
    }
}

impl HerdrWindow {
    /// The items the list draws, in its order: each listed checkout's
    /// uncommitted changes, unless it is detached, then its commits.
    fn changes_picks(&self) -> Vec<Pick> {
        let all = self.changes.all;
        self.changes
            .repos()
            .iter()
            .flat_map(|repo| &repo.checkouts)
            .filter(|shown| all || shown.rank() != Rank::Clean)
            .flat_map(|shown| {
                let checkout = &shown.checkout;
                let uncommitted = (checkout.dirty() && checkout.branch.is_some())
                    .then(|| Pick::Uncommitted(checkout.path.clone()));
                uncommitted
                    .into_iter()
                    .chain(checkout.commits.iter().map(|commit| Pick::Commit {
                        checkout: checkout.path.clone(),
                        sha: commit.sha.clone(),
                    }))
            })
            .collect()
    }

    /// Whether `pick` is what the review beside the list shows.
    pub(super) fn change_picked(&self, pick: &Pick) -> bool {
        self.changes.picked.as_ref() == Some(pick)
    }

    /// Shows `pick` beside the list, its uncommitted changes from `file`
    /// when given. Notes go to an agent whose pane is in the checkout.
    pub(super) fn pick_change(&mut self, pick: Pick, file: Option<String>, cx: &mut Context<Self>) {
        let Some(shown) = self
            .changes
            .repos()
            .iter()
            .flat_map(|repo| &repo.checkouts)
            .find(|shown| shown.checkout.path == pick.checkout())
            .cloned()
        else {
            return;
        };
        let checkout = &shown.checkout;
        let commit = match &pick {
            Pick::Uncommitted(_) if checkout.branch.is_none() => {
                self.show_flash(Flash::warning("A detached checkout cannot be reviewed"), cx);
                return;
            }
            Pick::Uncommitted(_) => None,
            Pick::Commit { sha, .. } => {
                let Ok(commit) = CommitId::try_from(sha.clone()) else {
                    self.show_flash(Flash::warning("That commit cannot be shown"), cx);
                    return;
                };
                Some(commit)
            }
        };
        let target = ReviewCheckout {
            repo_key: Some(checkout.common_dir.clone()),
            // A commit's review names the branch only to label it.
            branch: checkout.branch.clone().unwrap_or_else(|| "HEAD".to_owned()),
            checkout: Some(checkout.path.clone()),
        };
        let agent = self.changes_agent(&shown);
        if self.show_changes_review(&target, commit, file, agent, cx) {
            self.changes.picked = Some(pick);
            cx.notify();
        }
    }

    /// The agent of a pane working in `shown`'s checkout, if any.
    fn changes_agent(&self, shown: &Shown) -> Option<Agent> {
        let snapshot = self.local_snapshot()?;
        let inside = |directory: &Option<String>| {
            directory
                .as_deref()
                .is_some_and(|directory| Path::new(directory).starts_with(&shown.checkout.path))
        };
        snapshot
            .panes
            .iter()
            .filter(|pane| inside(&pane.cwd) || inside(&pane.foreground_cwd))
            .find_map(|pane| crate::review::agent_in(snapshot, &pane.pane_id))
    }

    /// Picks the list's first item while the section shows and nothing is
    /// picked yet, so the review beside it is never empty.
    pub(super) fn pick_first_change(&mut self, cx: &mut Context<Self>) {
        if self.changes.picked.is_some() || self.activity_section() != Some(ActivityPanel::Changes)
        {
            return;
        }
        if let Some(first) = self.changes_picks().into_iter().next() {
            self.pick_change(first, None, cx);
        }
    }

    /// Picks the item after the one picked, or before it.
    pub(super) fn step_change(&mut self, forward: bool, cx: &mut Context<Self>) {
        let picks = self.changes_picks();
        let at = self
            .changes
            .picked
            .as_ref()
            .and_then(|picked| picks.iter().position(|pick| pick == picked));
        let next = match (at, forward) {
            (None, _) => Some(0),
            (Some(at), true) => Some(at + 1),
            (Some(at), false) => at.checked_sub(1),
        };
        if let Some(pick) = next.and_then(|next| picks.get(next)).cloned() {
            self.pick_change(pick, None, cx);
        }
    }

    /// The handle the list holds the keyboard with, for ↑ and ↓.
    pub(super) fn changes_list_focus(&mut self, cx: &mut Context<Self>) -> FocusHandle {
        self.changes
            .focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone()
    }

    /// Picks `pick` from a click on its row, the list taking the keyboard.
    pub(super) fn click_change(
        &mut self,
        pick: Pick,
        file: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus = self.changes_list_focus(cx);
        window.focus(&focus, cx);
        self.pick_change(pick, file, cx);
    }
}
