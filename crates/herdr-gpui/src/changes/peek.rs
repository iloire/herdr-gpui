//! Looking at a changed file's lines without leaving the Changes section.
//! A file's row opens a peek under it: its uncommitted diff, read on the
//! background executor the way a review reads it, its first lines coloured.
//! A peek still open when a later pass finds the file's counts changed is
//! read again; one whose file is no longer changed closes.

use super::Shown;
use crate::{HerdrWindow, review::Peeked};
use gpui::Context;
use herdr_vcs::changes::FileChange;
use std::collections::HashMap;

/// Diff lines a peek shows; the rest are a review away.
pub(crate) const PEEK_LINES: usize = 40;

/// A file, by its checkout's path and its own.
pub(crate) type FileKey = (String, String);

/// One open peek.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Peek {
    /// The file's counts when it was read, to notice it changed.
    counts: (Option<u64>, Option<u64>),
    pub(crate) state: State,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Reading,
    Read(Peeked),
    Failed(String),
}

/// The open peeks of a window, and which checkouts show their noise.
#[derive(Default)]
pub(crate) struct Peeks {
    open: HashMap<FileKey, Peek>,
    /// Checkouts whose generated and binary files are listed one by one.
    pub(crate) noise_shown: std::collections::HashSet<String>,
}

impl Peeks {
    pub(crate) fn get(&self, key: &FileKey) -> Option<&Peek> {
        self.open.get(key)
    }

    /// Keys of open peeks whose file changed since it was read, after
    /// closing those whose file is no longer listed.
    fn stale(&mut self, shown: &[&Shown]) -> Vec<(FileKey, Option<String>)> {
        let mut stale = Vec::new();
        self.open.retain(|key, peek| {
            let Some((file, branch)) = find(shown, key) else {
                return false;
            };
            if (file.additions, file.deletions) != peek.counts {
                stale.push((key.clone(), branch));
            }
            true
        });
        stale
    }
}

fn find<'a>(shown: &[&'a Shown], key: &FileKey) -> Option<(&'a FileChange, Option<String>)> {
    let shown = shown.iter().find(|shown| shown.checkout.path == key.0)?;
    let file = shown
        .checkout
        .files
        .iter()
        .find(|file| file.path == key.1)?;
    Some((file, shown.checkout.branch.clone()))
}

impl HerdrWindow {
    /// Opens the peek under `key`'s row, or closes it.
    pub(crate) fn toggle_changes_peek(&mut self, key: FileKey, cx: &mut Context<Self>) {
        if self.changes.peeks.open.remove(&key).is_some() {
            cx.notify();
            return;
        }
        let shown = self.changes_shown();
        let Some((file, branch)) = find(&shown, &key) else {
            return;
        };
        let counts = (file.additions, file.deletions);
        self.changes.peeks.open.insert(
            key.clone(),
            Peek {
                counts,
                state: State::Reading,
            },
        );
        self.read_changes_peek(key, branch, cx);
    }

    /// Shows or hides a checkout's generated and binary files one by one.
    pub(crate) fn toggle_changes_noise(&mut self, checkout: String, cx: &mut Context<Self>) {
        let noise = &mut self.changes.peeks.noise_shown;
        if !noise.remove(&checkout) {
            noise.insert(checkout);
        }
        cx.notify();
    }

    /// Reads again the open peeks a pass found changed.
    pub(crate) fn refresh_changes_peeks(&mut self, cx: &mut Context<Self>) {
        let shown: Vec<Shown> = self.changes_shown().into_iter().cloned().collect();
        let refs: Vec<&Shown> = shown.iter().collect();
        for (key, branch) in self.changes.peeks.stale(&refs) {
            if let Some((file, _)) = find(&refs, &key)
                && let Some(peek) = self.changes.peeks.open.get_mut(&key)
            {
                peek.counts = (file.additions, file.deletions);
            }
            self.read_changes_peek(key, branch, cx);
        }
    }

    fn changes_shown(&self) -> Vec<&Shown> {
        self.changes
            .repos()
            .iter()
            .flat_map(|repo| &repo.checkouts)
            .collect()
    }

    fn read_changes_peek(&mut self, key: FileKey, branch: Option<String>, cx: &mut Context<Self>) {
        let input = crate::pull_request::Input {
            checkout: Some(key.0.clone()),
            repo_key: None,
            branch: branch.unwrap_or_default(),
        };
        let path = key.1.clone();
        let reading = cx.background_executor().spawn(async move {
            if input.branch.is_empty() {
                return State::Failed("A detached checkout has no branch to diff".into());
            }
            match crate::review::peek(&input, &path, PEEK_LINES) {
                Ok(peeked) => State::Read(peeked),
                Err(error) => State::Failed(error.to_string()),
            }
        });
        cx.spawn(async move |this, cx| {
            let state = reading.await;
            this.update(cx, |this, cx| {
                if let Some(peek) = this.changes.peeks.open.get_mut(&key) {
                    peek.state = state;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }
}
