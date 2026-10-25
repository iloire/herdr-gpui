//! The review beside the Changes list shows one commit or a checkout's
//! uncommitted work at a time, keeping how the last was drawn, and never
//! drops notes waiting to be sent. A commit's tab saved by an earlier build
//! still reads as that commit.
use super::{HerdrWindow, State, window};
use crate::review::view::Layout;
use crate::{
    browser::{Location, ReviewCheckout, Store, TabId},
    review::{CommitId, diff::Scope},
};
use gpui::{Entity, VisualTestContext};
use std::{path::Path, process::Command};

fn git(directory: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env_remove("GIT_DIR")
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// A repository on `main` whose last commit adds `added.rs`, with an
/// uncommitted edit to `base.rs`; the checkout and that commit.
fn repository(root: &Path) -> (ReviewCheckout, CommitId) {
    git(root, &["init", "-q", "-b", "main"]);
    std::fs::write(root.join("base.rs"), "fn a() {}\n").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "base"]);
    std::fs::write(root.join("added.rs"), "fn b() {}\n").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "feat: add b"]);
    std::fs::write(root.join("base.rs"), "fn a() { todo!() }\n").unwrap();
    let commit = CommitId::try_from(git(root, &["rev-parse", "HEAD"])).unwrap();
    let path = root.canonicalize().unwrap();
    let checkout = ReviewCheckout {
        repo_key: Some(path.join(".git").to_str().unwrap().into()),
        branch: "main".into(),
        checkout: Some(path.to_str().unwrap().into()),
    };
    (checkout, commit)
}

fn files(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext, id: TabId) -> Vec<String> {
    view.read_with(cx, |view, _| match &view.reviews[&id].state {
        State::Loaded(loaded) => loaded
            .diff
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect(),
        State::Failed(error) => panic!("the review failed: {error}"),
        State::Loading => panic!("the review is still loading"),
    })
}

/// Shows `checkout`'s `commit`, or its uncommitted work, beside the
/// Changes list and waits for its read; whether it was shown.
fn show(
    view: &Entity<HerdrWindow>,
    cx: &mut VisualTestContext,
    checkout: &ReviewCheckout,
    commit: Option<&CommitId>,
) -> bool {
    let shown = cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.show_changes_review(checkout, commit.cloned(), None, None, cx)
        })
    });
    cx.run_until_parked();
    shown
}

fn scope(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) -> Scope {
    view.read_with(cx, |view, _| view.reviews[&TabId::CHANGES].scope.clone())
}

#[gpui::test]
fn the_pane_shows_one_change_at_a_time_drawn_as_the_last(cx: &mut gpui::TestAppContext) {
    let root = tempfile::tempdir().unwrap();
    let (checkout, commit) = repository(root.path());
    let (view, cx) = window(cx, None);
    assert!(show(&view, cx, &checkout, Some(&commit)));
    assert_eq!(scope(&view, cx), Scope::Commit(commit.clone()));
    assert_eq!(files(&view, cx, TabId::CHANGES), ["added.rs"]);
    // Drawn side by side, then the uncommitted work is picked: still so.
    cx.update(|_, cx| {
        view.update(cx, |view, _| {
            view.reviews.get_mut(&TabId::CHANGES).unwrap().layout = Layout::Split;
        })
    });
    assert!(show(&view, cx, &checkout, None));
    assert_eq!(scope(&view, cx), Scope::Uncommitted);
    assert_eq!(files(&view, cx, TabId::CHANGES), ["base.rs"]);
    view.read_with(cx, |view, _| {
        assert_eq!(view.reviews[&TabId::CHANGES].layout, Layout::Split);
    });
}

#[gpui::test]
fn notes_waiting_keep_their_changes_shown(cx: &mut gpui::TestAppContext) {
    let root = tempfile::tempdir().unwrap();
    let (checkout, commit) = repository(root.path());
    let (view, cx) = window(cx, None);
    assert!(show(&view, cx, &checkout, None));
    cx.update(|_, cx| {
        view.update(cx, |view, _| {
            let anchor = crate::review::diff::Anchor::File {
                path: "base.rs".into(),
            };
            let note = crate::review::notes::Note::new(anchor, "Why?").unwrap();
            view.reviews
                .get_mut(&TabId::CHANGES)
                .unwrap()
                .notes
                .push(note);
        })
    });
    assert!(!show(&view, cx, &checkout, Some(&commit)));
    assert_eq!(scope(&view, cx), Scope::Uncommitted);
    view.read_with(cx, |view, _| {
        let flash = view.flash.as_ref().map(|(flash, _)| flash.text.to_string());
        assert_eq!(
            flash.as_deref(),
            Some("Send or remove this review's notes before picking other changes")
        );
    });
}

#[gpui::test]
fn a_saved_commit_tab_still_reads_its_commit(cx: &mut gpui::TestAppContext) {
    let root = tempfile::tempdir().unwrap();
    let (checkout, commit) = repository(root.path());
    let (view, cx) = window(cx, None);
    let id = cx.update(|_, cx| {
        let tab_scope = crate::browser::scope(&view.read(cx).endpoints[0]);
        Store::update(cx, |store| {
            store.open(
                tab_scope,
                "w0",
                Some(Location::Review {
                    checkout,
                    commit: Some(commit.clone()),
                }),
                None,
            )
        })
        .unwrap()
    });
    cx.update(|_, cx| view.update(cx, |view, cx| view.poll_reviews(cx)));
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(view.reviews[&id].scope, Scope::Commit(commit.clone()));
    });
    assert_eq!(files(&view, cx, id), ["added.rs"]);
}
