//! What a folder's Changes shows beside its list, as Git clients do: the
//! item picked on the left, a commit or a checkout's uncommitted changes,
//! read in the review on the right (`review::view::pane`).

use super::Shown;
use crate::{
    HerdrWindow,
    browser::ReviewCheckout,
    review::{Agent, CommitId},
    window::Flash,
};
use gpui::prelude::*;
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

    /// Shows the checkout at `path` in the review, unless something of it
    /// already is: its uncommitted changes, else its latest commit.
    pub(crate) fn pick_checkout_change(&mut self, path: &str, cx: &mut Context<Self>) {
        if self
            .changes
            .picked
            .as_ref()
            .is_some_and(|picked| picked.checkout() == path)
        {
            return;
        }
        let Some(checkout) = self
            .changes
            .repos()
            .iter()
            .flat_map(|repo| &repo.checkouts)
            .map(|shown| &shown.checkout)
            .find(|checkout| checkout.path == path)
        else {
            return;
        };
        let pick = if checkout.dirty() && checkout.branch.is_some() {
            Pick::Uncommitted(checkout.path.clone())
        } else if let Some(commit) = checkout.commits.first() {
            Pick::Commit {
                checkout: checkout.path.clone(),
                sha: commit.sha.clone(),
            }
        } else {
            return;
        };
        self.pick_change(pick, None, cx);
    }

    /// Picks `pick` from a click on its row.
    pub(super) fn click_change(
        &mut self,
        pick: Pick,
        file: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.pick_change(pick, file, cx);
    }
}
