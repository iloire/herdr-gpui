#![allow(clippy::unwrap_used)]
// Not a glob: the parent's `gpui` imports would shadow `#[test]`.
use crate::{
    HerdrWindow,
    browser::{Location, Pick, Store, TabId},
    editor::EditorTarget,
    sidebar::ActivityPanel,
};
use gpui::{Entity, TestAppContext, VisualTestContext};
use std::sync::Arc;

mod opening;

fn window(cx: &mut TestAppContext) -> (Entity<HerdrWindow>, &mut VisualTestContext) {
    cx.add_window_view(|window, cx| {
        let mut view = crate::sidebar::layout_tests::fixture_window(window, cx);
        view.config.sidebar_style.activity_bar = true;
        let mut shown = serde_json::to_value(crate::sidebar::layout_tests::snapshot(1)).unwrap();
        shown["focused_workspace_id"] = "w0".into();
        view.live.snapshot = Some(Arc::new(serde_json::from_value(shown).unwrap()));
        view.live.status = crate::state::ConnectionStatus::Connected;
        view
    })
}

fn show(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext, panel: ActivityPanel) {
    cx.update(|window, cx| {
        view.update(cx, |view, cx| view.show_activity_panel(panel, window, cx));
    });
}

/// Shows the window's folder at its Files.
fn files(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open_shown_folder(crate::folders::Part::Files, window, cx)
        });
    });
    assert!(view.read_with(cx, |view, _| view.files_view()));
}

/// Whether the strip lists tab `id`, and whether its group shows it.
fn listed(view: &Entity<HerdrWindow>, id: TabId, cx: &mut VisualTestContext) -> (bool, bool) {
    view.read_with(cx, |view, _| {
        let group = view.active_group().unwrap();
        let pick = Pick::Page(id);
        let shows = view.shown_pick_for_test(group) == Some(pick.clone());
        (view.group_lists(group, &pick), shows)
    })
}

#[gpui::test]
fn file_tabs_stay_in_a_folders_files(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.rs");
    std::fs::write(&path, "fn main() {}\n").unwrap();
    let (view, cx) = window(cx);
    files(&view, cx);
    let target = EditorTarget { path, line: None };
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_code_view(&target, window, cx)));
    let id = view.read_with(cx, |view, _| *view.code_views.keys().next().unwrap());
    cx.update(|_, cx| view.update(cx, |view, cx| view.sync_file_tabs(cx)));
    assert_eq!(listed(&view, id, cx), (true, true));

    // Workspaces neither lists nor shows it; the group goes back to the
    // terminal it follows.
    show(&view, cx, ActivityPanel::Workspaces);
    assert_eq!(listed(&view, id, cx), (false, false));
    // Another section across the body does not bring it back either.
    show(&view, cx, ActivityPanel::Overview);
    assert_eq!(listed(&view, id, cx), (false, false));

    files(&view, cx);
    assert_eq!(listed(&view, id, cx), (true, true));
}

#[gpui::test]
fn a_page_an_agent_opened_stays_with_the_terminals(cx: &mut TestAppContext) {
    let dir = tempfile::Builder::new().prefix("plans").tempdir().unwrap();
    let path = dir.path().join("plan.md");
    std::fs::write(&path, "# Plan\n").unwrap();
    let (view, cx) = window(cx);
    let id = cx.update(|window, cx| {
        let scope = crate::browser::scope(&view.read(cx).endpoints[0]);
        let file = crate::browser::LocalFile::new(&path, None).unwrap();
        let id = Store::update(cx, |store| {
            store.open(
                scope,
                "w0",
                Some(Location::Local { file }),
                Some("p1".into()),
            )
        })
        .unwrap();
        view.update(cx, |view, cx| {
            view.show_browser_tab(id, window, cx);
            view.sync_file_tabs(cx);
        });
        id
    });
    show(&view, cx, ActivityPanel::Workspaces);
    assert!(listed(&view, id, cx).0);
    // A folder's Files lists only the files opened from its tree; the page
    // waits under Terminals, with the agent that opened it.
    files(&view, cx);
    assert!(!listed(&view, id, cx).0);
    view.read_with(cx, |view, cx| {
        let group = view.active_group().unwrap();
        assert_eq!(view.group_shown(group, cx), crate::browser::Shown::Empty);
    });
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open_shown_folder(crate::folders::Part::Terminals, window, cx)
        })
    });
    assert!(listed(&view, id, cx).0);
}
