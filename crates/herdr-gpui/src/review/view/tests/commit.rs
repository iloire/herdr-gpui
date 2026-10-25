//! A commit opened from the Changes section gets a review tab of its own,
//! named by the commit, beside the checkout's review of uncommitted work,
//! and stays on that commit.
use super::{HerdrWindow, State, window};
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

/// Opens `checkout`'s review of `commit`, or of its uncommitted work, as
/// Changes does, and waits for its read; its tab.
fn open(
    view: &Entity<HerdrWindow>,
    cx: &mut VisualTestContext,
    checkout: &ReviewCheckout,
    commit: Option<&CommitId>,
) -> TabId {
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open_review_of(
                checkout.clone(),
                commit.cloned(),
                0,
                "w0".into(),
                None,
                None,
                window,
                cx,
            );
        })
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        cx.global::<Store>()
            .tabs()
            .find(|tab| {
                matches!(&tab.location, Some(Location::Review { commit: open, .. }) if open.as_ref() == commit)
            })
            .map(|tab| tab.id)
            .unwrap()
    })
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

#[gpui::test]
fn a_commit_opens_in_its_own_tab_and_stays_on_it(cx: &mut gpui::TestAppContext) {
    let root = tempfile::tempdir().unwrap();
    let (checkout, commit) = repository(root.path());
    let (view, cx) = window(cx, None);
    let shown = open(&view, cx, &checkout, Some(&commit));
    assert_eq!(files(&view, cx, shown), ["added.rs"]);
    view.read_with(cx, |view, cx| {
        let review = &view.reviews[&shown];
        assert_eq!(review.scope, Scope::Commit(commit.clone()));
        let summary = review.loaded().and_then(|loaded| loaded.summary.clone());
        assert_eq!(
            summary.map(|summary| summary.subject),
            Some("feat: add b".into())
        );
        let title = cx.global::<Store>().get(shown).map(|tab| tab.title.clone());
        assert_eq!(
            title.as_deref(),
            Some(format!("Commit \u{00b7} {}", commit.short()).as_str())
        );
    });
    // The scope switch never takes a commit's tab off its commit.
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.set_review_scope(shown, Scope::Branch, cx)
        });
    });
    view.read_with(cx, |view, _| {
        assert_eq!(view.reviews[&shown].scope, Scope::Commit(commit.clone()));
    });
    // Uncommitted work is another tab; the commit's own is brought back.
    let uncommitted = open(&view, cx, &checkout, None);
    assert_ne!(uncommitted, shown);
    assert_eq!(files(&view, cx, uncommitted), ["base.rs"]);
    assert_eq!(open(&view, cx, &checkout, Some(&commit)), shown);
    let reviews = cx.update(|_, cx| {
        cx.global::<Store>()
            .tabs()
            .filter(|tab| matches!(tab.location, Some(Location::Review { .. })))
            .count()
    });
    assert_eq!(reviews, 2);
}
