//! The review beside a folder's Changes, as Git clients show it: the first
//! item is picked when the part shows, picking another shows it in place,
//! and nothing leaves Folders or opens a tab.
use super::*;
use crate::{
    HerdrWindow,
    browser::{Location, Store, TabId},
    changes::pane::Pick,
    sidebar::ActivityPanel,
};
use gpui::{Entity, VisualTestContext};
use std::time::Instant;

/// Commits `name`, a new file; the commit's full name.
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

/// A checkout with two commits after its first and an uncommitted edit,
/// as a pass reads it: the checkout and its commits, newest first.
fn checkout_read(path: &Path) -> (Shown, Vec<String>) {
    repository(path);
    let first = commit(path, "first.rs");
    let second = commit(path, "second.rs");
    std::fs::write(path.join("a"), "edited\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let session = &mut herdr_vcs::host::Session::local();
    let located =
        herdr_vcs::changes::locate(session, path.to_str().unwrap(), deadline, &|| false).unwrap();
    let checkout = herdr_vcs::changes::read(session, located, deadline, &|| false).unwrap();
    (shown(checkout, &["w0"]), vec![second, first])
}

/// What the review beside the list shows: what is picked and its files.
fn showing(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) -> (Option<Pick>, Vec<String>) {
    view.read_with(cx, |view, _| {
        let files = view
            .reviews
            .get(&TabId::CHANGES)
            .map(|review| review.file_paths())
            .unwrap_or_default();
        (view.changes.picked.clone(), files)
    })
}

fn pick(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext, pick: Pick) {
    cx.update(|_, cx| view.update(cx, |view, cx| view.pick_change(pick, None, cx)));
    cx.run_until_parked();
}

#[gpui::test]
fn a_folders_changes_show_what_is_picked_beside_it_and_never_leave(cx: &mut gpui::TestAppContext) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap();
    let (shown, commits) = checkout_read(&path);
    let checkout = shown.checkout.path.clone();
    let read = Read {
        repos: group(vec![shown]),
        failed: Vec::new(),
    };
    let (view, cx) = window(cx, read);
    let at = |sha: &str| Pick::Commit {
        checkout: checkout.clone(),
        sha: sha.to_owned(),
    };

    // The first item, the uncommitted edit, is picked at once.
    assert_eq!(
        showing(&view, cx),
        (Some(Pick::Uncommitted(checkout.clone())), vec!["a".into()])
    );
    // A commit picked shows its files in place.
    pick(&view, cx, at(&commits[1]));
    assert_eq!(
        showing(&view, cx),
        (Some(at(&commits[1])), vec!["first.rs".into()])
    );
    pick(&view, cx, at(&commits[0]));
    assert_eq!(
        showing(&view, cx),
        (Some(at(&commits[0])), vec!["second.rs".into()])
    );

    // The section stays, and no tab was opened in a workspace.
    view.read_with(cx, |view, cx| {
        assert_eq!(view.activity_section(), Some(ActivityPanel::Folders));
        // No store at all means no tab was ever opened.
        let reviews = cx.try_global::<Store>().map_or(0, |store| {
            store
                .tabs()
                .filter(|tab| matches!(tab.location, Some(Location::Review { .. })))
                .count()
        });
        assert_eq!(reviews, 0);
    });
}
