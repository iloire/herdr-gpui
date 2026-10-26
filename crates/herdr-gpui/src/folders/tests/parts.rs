use super::page::{bounds, click, draw, endpoint, fixture};
use crate::{HerdrWindow, browser::Pick, folders::Part};
use gpui::{Entity, VisualTestContext};
use std::sync::Arc;

/// The daemon's answer to a switch: `workspace` is now the one shown.
fn focus(view: &Entity<HerdrWindow>, workspace: &str, cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            if let Some(snapshot) = view.live.snapshot.as_deref() {
                let mut shown = snapshot.clone();
                shown.focused_workspace_id = Some(workspace.into());
                view.live.snapshot = Some(Arc::new(shown));
            }
            cx.notify();
        })
    });
    draw(cx);
}

fn part(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) -> (Part, bool) {
    cx.update(|_, cx| {
        let view = view.read(cx);
        (view.folders.part, view.folders_column())
    })
}

#[gpui::test]
fn terminals_stay_in_folders_beside_the_terminal_area(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    click(cx, "folder-part-terminals");
    focus(&view, "w1", cx);
    cx.update(|window, cx| {
        let view = view.read(cx);
        assert!(view.folders_column(), "Folders stays, as a column");
        assert_eq!(view.activity_section(), None, "the terminals take the body");
        assert!(
            view.focus.is_focused(window),
            "the keyboard is in the terminal"
        );
    });
    let list = bounds(cx, "folders-column").unwrap();
    let terminal = bounds(cx, "terminal").unwrap();
    assert!(
        list.right() <= terminal.left(),
        "the list stands left of the terminals"
    );
    assert!(bounds(cx, "folders").is_none(), "no page");
    assert!(
        bounds(cx, "folder-part-terminals").is_some(),
        "the parts stay listed"
    );
    assert!(bounds(cx, "files").is_none());
}

#[gpui::test]
fn files_shows_only_files_beside_the_tree(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    // w0 holds the terminal tabs.
    click(cx, &format!("folder-{host}-w0"));
    click(cx, "folder-part-terminals");
    focus(&view, "w0", cx);
    let terminals = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let view = view.read(cx);
            let group = view.active_group().unwrap();
            view.group_tabs(group, cx)
                .iter()
                .filter(|tab| matches!(tab, Pick::Herdr(_)))
                .count()
        })
    };
    assert_eq!(terminals(cx), 2, "Terminals lists them");
    assert!(bounds(cx, "terminal").is_some());
    click(cx, "folder-part-files");
    assert!(
        cx.update(|_, cx| view.read(cx).files_view()),
        "file tabs list here"
    );
    assert_eq!(terminals(cx), 0, "Files lists none");
    let list = bounds(cx, "folders-list").unwrap();
    let tree = bounds(cx, "files").unwrap();
    let prompt = bounds(cx, "stand-in").unwrap();
    assert!(list.right() <= tree.left() && tree.right() <= prompt.left());
    assert!(
        bounds(cx, "terminal").is_none(),
        "the terminal waits under Terminals"
    );
    // Home is a page again, and the file tabs go back to Workspaces' rules.
    click(cx, "folder-part-home");
    assert!(bounds(cx, "folder-home").is_some());
    assert!(bounds(cx, "files").is_none());
    assert!(!cx.update(|_, cx| view.read(cx).files_view()));
}

#[gpui::test]
fn each_folder_opens_at_the_part_it_was_left_on(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    // w1 leads, as it waits on the user: leave it at its Terminals.
    click(cx, "folder-part-terminals");
    focus(&view, "w1", cx);
    // w0 was never opened, so it follows the part shown.
    click(cx, &format!("folder-{host}-w0"));
    focus(&view, "w0", cx);
    assert_eq!(part(&view, cx), (Part::Terminals, true));
    click(cx, "folder-part-home");
    assert_eq!(part(&view, cx), (Part::Home, false));
    // Back to w1: its Terminals, and back to w0: its Home.
    click(cx, &format!("folder-{host}-w1"));
    assert_eq!(part(&view, cx), (Part::Terminals, true));
    focus(&view, "w1", cx);
    click(cx, &format!("folder-{host}-w0"));
    assert_eq!(part(&view, cx), (Part::Home, false));
    assert!(bounds(cx, "folder-home").is_some());
}

#[gpui::test]
fn the_list_follows_a_workspace_switched_from_the_keyboard(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    click(cx, "folder-part-terminals");
    focus(&view, "w1", cx);
    let unfolded = |cx: &mut VisualTestContext, workspace: &str| {
        let row = bounds(cx, &format!("folder-{host}-{workspace}")).unwrap();
        let parts = bounds(cx, "folder-part-terminals").unwrap();
        parts.top() > row.top() && parts.top() - row.bottom() < gpui::px(40.)
    };
    assert!(unfolded(cx, "w1"));
    focus(&view, "w0", cx);
    assert!(
        unfolded(cx, "w0"),
        "the parts hang under the workspace shown"
    );
}
