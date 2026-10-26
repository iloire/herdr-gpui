use super::{draw, window};
use crate::{HerdrWindow, browser::TabId, editor::EditorTarget};
use gpui::{Entity, TestAppContext, VisualTestContext};
use std::{path::Path, time::Duration};

/// Opens `path` in a code tab at line 2, ready to type.
fn open<'a>(
    path: &Path,
    cx: &'a mut TestAppContext,
) -> (Entity<HerdrWindow>, TabId, &'a mut VisualTestContext) {
    let (view, cx) = window(cx);
    let target = EditorTarget {
        path: path.to_owned(),
        line: Some(2),
    };
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_code_view(&target, window, cx)));
    cx.run_until_parked();
    draw(cx);
    let id = view.read_with(cx, |view, _| *view.code_views.keys().next().unwrap());
    (view, id, cx)
}

fn file(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

fn unsaved(view: &Entity<HerdrWindow>, id: TabId, cx: &mut VisualTestContext) -> bool {
    view.read_with(cx, |view, cx| view.unsaved_code_edits(id, cx))
}

#[gpui::test]
fn a_code_tab_opens_ready_to_type_and_saves_on_cmd_s(cx: &mut TestAppContext) {
    let dir = tempfile::Builder::new().prefix("edit").tempdir().unwrap();
    let path = dir.path().join("main.rs");
    std::fs::write(&path, "fn a() {}\nfn b() {}\n").unwrap();
    let (view, id, cx) = open(&path, cx);
    assert!(cx.debug_bounds("code-edit").is_some());
    // The cursor starts on the line the tab opened at.
    cx.simulate_input("// ");
    assert!(unsaved(&view, id, cx), "the tab is marked until saved");
    cx.simulate_keystrokes("cmd-s");
    cx.run_until_parked();
    assert_eq!(file(&path), "fn a() {}\n// fn b() {}\n");
    assert!(!unsaved(&view, id, cx));
}

#[gpui::test]
fn edits_save_themselves_once_typing_pauses(cx: &mut TestAppContext) {
    let dir = tempfile::Builder::new().prefix("edit").tempdir().unwrap();
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, "one\ntwo\n").unwrap();
    let (view, id, cx) = open(&path, cx);
    cx.simulate_input("x");
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(file(&path), "one\ntwo\n", "not while typing");
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(file(&path), "one\nxtwo\n");
    assert!(!unsaved(&view, id, cx));
}

#[gpui::test]
fn closing_a_tab_saves_its_edits(cx: &mut TestAppContext) {
    let dir = tempfile::Builder::new().prefix("edit").tempdir().unwrap();
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, "one\ntwo\n").unwrap();
    let (view, id, cx) = open(&path, cx);
    cx.simulate_input("y");
    cx.update(|window, cx| view.update(cx, |view, cx| view.close_browser_tab(id, window, cx)));
    cx.update(|_, cx| view.update(cx, |view, cx| view.poll_code_views(cx)));
    cx.run_until_parked();
    assert!(view.read_with(cx, |view, _| view.code_views.is_empty()));
    assert_eq!(file(&path), "one\nytwo\n");
}

#[gpui::test]
fn an_agents_change_under_edits_is_never_overwritten_unasked(cx: &mut TestAppContext) {
    let dir = tempfile::Builder::new().prefix("edit").tempdir().unwrap();
    let path = dir.path().join("main.rs");
    std::fs::write(&path, "one\ntwo\n").unwrap();
    let (view, id, cx) = open(&path, cx);
    cx.simulate_input("mine ");
    std::fs::write(&path, "theirs\n").unwrap();
    cx.simulate_keystrokes("cmd-s");
    cx.run_until_parked();
    assert_eq!(file(&path), "theirs\n");
    let editor = view.read_with(cx, |view, _| view.code_editor(id).unwrap().clone());
    assert!(editor.read_with(cx, |editor, _| editor.conflicted()));
    // Waiting for the choice, autosave leaves the file alone.
    cx.executor().advance_clock(Duration::from_secs(5));
    cx.run_until_parked();
    assert_eq!(file(&path), "theirs\n");
    draw(cx);
    assert!(cx.debug_bounds("code-edit-overwrite").is_some());

    // Revert takes the file's text, as a step undo takes back.
    editor.update(cx, |editor, cx| editor.revert(cx));
    cx.run_until_parked();
    assert!(!unsaved(&view, id, cx));
    assert!(!editor.read_with(cx, |editor, _| editor.conflicted()));
    cx.simulate_keystrokes("cmd-z");
    assert!(unsaved(&view, id, cx), "undo brings the edits back");

    // Now saved over the file, as asked.
    std::fs::write(&path, "theirs again\n").unwrap();
    editor.update(cx, |editor, cx| editor.save(false, cx));
    cx.run_until_parked();
    assert!(editor.read_with(cx, |editor, _| editor.conflicted()));
    editor.update(cx, |editor, cx| editor.save(true, cx));
    cx.run_until_parked();
    assert_eq!(file(&path), "one\nmine two\n");
}

#[gpui::test]
fn a_change_on_disk_is_taken_while_nothing_is_unsaved(cx: &mut TestAppContext) {
    let dir = tempfile::Builder::new().prefix("edit").tempdir().unwrap();
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, "one\ntwo\n").unwrap();
    let (view, id, cx) = open(&path, cx);
    std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
    let editor = view.read_with(cx, |view, _| view.code_editor(id).unwrap().clone());
    editor.update(cx, |editor, cx| editor.disk_changed(cx));
    cx.run_until_parked();
    assert_eq!(editor.read_with(cx, |editor, _| editor.line_count()), 3);
    assert!(!unsaved(&view, id, cx));
}
