//! The review the Changes section shows beside its list, as Git clients do:
//! a commit, or a checkout's uncommitted changes, picked on the left and
//! read on the right without leaving the section. It is one review, under
//! [`TabId::CHANGES`] rather than a tab, made anew for each pick and keeping
//! how the last one was drawn.
use super::{Agent, Review, Scope};
use crate::{
    HerdrWindow,
    browser::{ReviewCheckout, TabId},
    pull_request::Input,
    review::{CommitId, diff::Anchor},
    window::Flash,
};
use gpui::{prelude::*, *};

impl Review {
    /// The paths of the files listed, once read.
    #[cfg(test)]
    pub(crate) fn file_paths(&self) -> Vec<String> {
        self.loaded()
            .map(|loaded| {
                loaded
                    .diff
                    .files
                    .iter()
                    .map(|file| file.path.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Whether notes are being written or wait to be sent, which belong to
    /// these changes and would be lost with them. A note opened with nothing
    /// typed yet holds nothing: a click on a header opens one.
    pub(super) fn holds_notes(&self, cx: &App) -> bool {
        let typing = (self.draft.is_some() || self.editing.is_some())
            && !self.input.read(cx).text().trim().is_empty();
        typing || self.notes.iter().any(|note| note.sent.is_none())
    }
}

impl HerdrWindow {
    /// Shows `checkout`'s `commit`, or else its uncommitted changes from
    /// `file`, beside the Changes list, reading them off the UI thread.
    /// Refused while the review shown there holds notes. Whether it shows.
    pub(crate) fn show_changes_review(
        &mut self,
        checkout: &ReviewCheckout,
        commit: Option<CommitId>,
        file: Option<String>,
        agent: Option<Agent>,
        cx: &mut Context<Self>,
    ) -> bool {
        let id = TabId::CHANGES;
        let input = Input::from(checkout);
        let shown = self.reviews.get(&id);
        let same = shown.is_some_and(|review| {
            review.checkout == input
                && match (&commit, &review.scope) {
                    (Some(commit), Scope::Commit(open)) => commit == open,
                    (None, Scope::Uncommitted | Scope::Branch) => true,
                    _ => false,
                }
        });
        if same {
            // The changes shown, read again at the file asked for.
            if let (Some(path), Some(review)) = (file, self.reviews.get_mut(&id)) {
                review.restore = Some(Anchor::File { path });
                self.load_review(id, cx);
            }
            return true;
        }
        if shown.is_some_and(|review| review.holds_notes(cx)) {
            self.show_flash(
                Flash::warning("Send or remove this review's notes before picking other changes"),
                cx,
            );
            return false;
        }
        let mut review = self.new_review(id, input, agent, cx);
        if let Some(last) = self.reviews.get(&id) {
            review.layout = last.layout;
            review.ignore_whitespace = last.ignore_whitespace;
            review.split_ratio = last.split_ratio;
            review.files_shown = last.files_shown;
            review.notes_shown = last.notes_shown;
            review.hide_viewed = last.hide_viewed;
        }
        review.scope = commit.map_or(Scope::Uncommitted, Scope::Commit);
        review.restore = file.map(|path| Anchor::File { path });
        self.reviews.insert(id, review);
        self.load_review(id, cx);
        true
    }

    /// The review beside the Changes list, or what to pick when there is
    /// none yet.
    pub(crate) fn render_changes_review(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.reviews.contains_key(&TabId::CHANGES) {
            return self.render_review_frame(TabId::CHANGES, "changes-review".into(), cx);
        }
        div()
            .debug_selector(|| "changes-review-empty".into())
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(self.theme.muted))
            .child("Pick a commit or a file to see its changes")
            .into_any_element()
    }
}
