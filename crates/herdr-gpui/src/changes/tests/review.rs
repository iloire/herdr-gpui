//! A commit clicked in Changes shows its review: the section gives way to
//! the workspace's view, which holds the tab, Back returns to the section,
//! and the checkout's next commit moves that tab rather than stacking another.
use super::*;
use crate::{
    HerdrWindow,
    browser::{Location, Store},
    sidebar::ActivityPanel,
    state::ConnectionStatus,
};
use gpui::{Entity, VisualTestContext};
use herdr_vcs::changes::Commit;
use std::sync::Arc;

/// Commits `name` adding a file of that name; its full object name.
fn commit(directory: &Path, name: &str) -> String {
    std::fs::write(directory.join(name), format!("{name}\n")).unwrap();
    git(directory, &["add", "-A"]);
    git(directory, &["commit", "-qm", name]);
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// The fixture window with the activity bar, connected to this machine's
/// daemon, showing the Changes section.
fn window(cx: &mut gpui::TestAppContext) -> (Entity<HerdrWindow>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut view = crate::sidebar::layout_tests::fixture_window(window, cx);
        view.config.sidebar_style.activity_bar = true;
        let mut listing = crate::sidebar::layout_tests::snapshot(2);
        listing.focused_workspace_id = Some("w0".into());
        view.live.snapshot = Some(Arc::new(listing));
        view.live.status = ConnectionStatus::Connected;
        view.live.local_daemon_peer = true;
        view
    });
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.show_activity_panel(ActivityPanel::Changes, window, cx);
        })
    });
    (view, cx)
}

fn click(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext, shown: &Shown, commit: &Commit) {
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.review_changes_commit(shown, commit, window, cx);
        })
    });
    cx.run_until_parked();
}

/// The review tabs open: each one's commit.
fn commit_tabs(cx: &mut VisualTestContext) -> Vec<(crate::browser::TabId, Option<String>)> {
    cx.update(|_, cx| {
        cx.global::<Store>()
            .tabs()
            .filter_map(|tab| match &tab.location {
                Some(Location::Review { commit, .. }) => {
                    Some((tab.id, commit.as_ref().map(|c| c.as_str().to_owned())))
                }
                _ => None,
            })
            .collect()
    })
}

#[gpui::test]
fn a_clicked_commit_shows_its_review_and_the_next_one_moves_that_tab(
    cx: &mut gpui::TestAppContext,
) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap();
    repository(&path);
    let first = commit(&path, "first.rs");
    let second = commit(&path, "second.rs");
    let at = |sha: &str| Commit {
        sha: sha.to_owned(),
        subject: String::new(),
        author: String::new(),
        time: 0,
    };
    let mut repo = checkout(path.to_str().unwrap(), path.join(".git").to_str().unwrap());
    repo.commits = vec![at(&second), at(&first)];
    let shown = shown(repo, &["w0"]);
    let (view, cx) = window(cx);
    assert_eq!(
        cx.update(|_, cx| view.read(cx).activity_section()),
        Some(ActivityPanel::Changes)
    );

    click(&view, cx, &shown, &shown.checkout.commits[0]);
    // The section no longer covers the tab, and Back returns to it.
    view.read_with(cx, |view, _| {
        assert_eq!(view.activity_section(), None);
        assert_eq!(view.activity.return_to, Some(ActivityPanel::Changes));
    });
    let tabs = commit_tabs(cx);
    assert_eq!(tabs, [(tabs[0].0, Some(second.clone()))]);
    let tab = tabs[0].0;

    // Another commit of the checkout takes that tab over and reads anew.
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.show_activity_panel(ActivityPanel::Changes, window, cx);
        })
    });
    click(&view, cx, &shown, &shown.checkout.commits[1]);
    assert_eq!(commit_tabs(cx), [(tab, Some(first.clone()))]);
    view.read_with(cx, |view, cx| {
        assert_eq!(view.activity_section(), None);
        // Made again on the new commit, whose name its tab now carries.
        assert!(view.reviews.contains_key(&tab));
        let title = cx.global::<Store>().get(tab).map(|tab| tab.title.clone());
        assert_eq!(title, Some(format!("Commit \u{00b7} {}", &first[..7])));
    });
}
