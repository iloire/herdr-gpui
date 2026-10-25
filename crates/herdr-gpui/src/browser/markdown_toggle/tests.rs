#![allow(clippy::unwrap_used)]
// Not a glob: the parent's `gpui` imports would shadow `#[test]`.
use super::renders;
use crate::{
    HerdrWindow,
    browser::{Location, Store, TabId},
    editor::EditorTarget,
};
use gpui::{Entity, TestAppContext, VisualTestContext};
use std::{path::Path, sync::Arc};

fn window(cx: &mut TestAppContext) -> (Entity<HerdrWindow>, &mut VisualTestContext) {
    cx.add_window_view(|window, cx| {
        let mut view = crate::sidebar::layout_tests::fixture_window(window, cx);
        let mut shown = serde_json::to_value(crate::sidebar::layout_tests::snapshot(1)).unwrap();
        shown["focused_workspace_id"] = "w0".into();
        view.live.snapshot = Some(Arc::new(serde_json::from_value(shown).unwrap()));
        view
    })
}

fn location(view: &Entity<HerdrWindow>, id: TabId, cx: &mut VisualTestContext) -> Location {
    view.read_with(cx, |_, cx| {
        cx.global::<Store>()
            .get(id)
            .unwrap()
            .location
            .clone()
            .unwrap()
    })
}

fn tabs(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) -> usize {
    view.read_with(cx, |view, cx| {
        let (scope, workspace) = view.browser_key().unwrap();
        cx.global::<Store>()
            .in_workspace(&scope, &workspace)
            .count()
    })
}

#[test]
fn only_markdown_renders_and_only_where_pages_are_shown() {
    let pages = crate::browser::EMBEDDED;
    assert_eq!(renders(Path::new("/r/docs/plan.md")), pages);
    assert_eq!(renders(Path::new("/r/README.MARKDOWN")), pages);
    assert!(!renders(Path::new("/r/src/main.rs")));
}

#[gpui::test]
fn a_markdown_tab_turns_between_source_and_rendered_in_place(cx: &mut TestAppContext) {
    if !crate::browser::EMBEDDED {
        return;
    }
    // Pages refuse hidden folders, and `tempdir` names its own `.tmp…`.
    let dir = tempfile::Builder::new().prefix("plans").tempdir().unwrap();
    let path = dir.path().join("plan.md");
    std::fs::write(&path, "# Plan\n").unwrap();
    let (view, cx) = window(cx);
    let target = EditorTarget {
        path: path.clone(),
        line: None,
    };
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_code_view(&target, window, cx)));
    cx.run_until_parked();
    let id = view.read_with(cx, |view, _| *view.code_views.keys().next().unwrap());
    assert!(view.read_with(cx, |view, cx| view.markdown_toggles(id, cx)));

    cx.update(|window, cx| view.update(cx, |view, cx| view.toggle_markdown(id, window, cx)));
    let Location::Local { file } = location(&view, id, cx) else {
        panic!("not rendered");
    };
    assert_eq!(file.path(), path);
    assert!(view.read_with(cx, |view, _| view.code_views.is_empty()));

    cx.update(|window, cx| view.update(cx, |view, cx| view.toggle_markdown(id, window, cx)));
    cx.run_until_parked();
    assert!(matches!(location(&view, id, cx), Location::Code { .. }));
    assert!(view.read_with(cx, |view, _| view.code_views.contains_key(&id)));
    assert_eq!(tabs(&view, cx), 1);

    // Opening it from Files turns the same tab back to rendered.
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_markdown(&path, window, cx)));
    assert!(matches!(location(&view, id, cx), Location::Local { .. }));
    assert_eq!(tabs(&view, cx), 1);
}

#[gpui::test]
fn source_code_offers_no_toggle(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.rs");
    std::fs::write(&path, "fn main() {}\n").unwrap();
    let (view, cx) = window(cx);
    let target = EditorTarget { path, line: None };
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_code_view(&target, window, cx)));
    let id = view.read_with(cx, |view, _| *view.code_views.keys().next().unwrap());
    assert!(!view.read_with(cx, |view, cx| view.markdown_toggles(id, cx)));
}
