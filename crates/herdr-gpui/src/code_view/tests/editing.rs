use super::{draw, window};
use crate::editor::EditorTarget;
use gpui::{TestAppContext, VisualTestContext};

/// Opens `path` in a code tab, then `i` makes it editable.
fn edit<'a>(
    path: &std::path::Path,
    cx: &'a mut TestAppContext,
) -> (gpui::Entity<crate::HerdrWindow>, &'a mut VisualTestContext) {
    let (view, cx) = window(cx);
    let target = EditorTarget {
        path: path.to_owned(),
        line: Some(2),
    };
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_code_view(&target, window, cx)));
    cx.run_until_parked();
    draw(cx);
    cx.simulate_keystrokes("i");
    cx.run_until_parked();
    draw(cx);
    (view, cx)
}

#[gpui::test]
fn i_edits_the_file_and_save_writes_it(cx: &mut TestAppContext) {
    let dir = tempfile::Builder::new().prefix("edit").tempdir().unwrap();
    let path = dir.path().join("main.rs");
    std::fs::write(&path, "fn a() {}\nfn b() {}\n").unwrap();
    let (view, cx) = edit(&path, cx);
    assert!(cx.debug_bounds("code-edit").is_some());
    // The cursor starts on the line the tab showed.
    cx.simulate_input("// ");
    cx.simulate_keystrokes("cmd-s");
    cx.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "fn a() {}\n// fn b() {}\n"
    );
    // Typed text never reached the window's own keys: the tab stays editing.
    view.read_with(cx, |view, cx| {
        let id = *view.code_views.keys().next().unwrap();
        let editor = view.code_editor(id).unwrap().read(cx);
        assert!(!editor.dirty());
    });
    cx.simulate_keystrokes("escape");
    view.read_with(cx, |view, _| {
        let id = *view.code_views.keys().next().unwrap();
        assert!(
            view.code_editor(id).is_none(),
            "Escape leaves a saved editor"
        );
    });
}

#[gpui::test]
fn saving_over_an_agents_change_asks_first(cx: &mut TestAppContext) {
    let dir = tempfile::Builder::new().prefix("edit").tempdir().unwrap();
    let path = dir.path().join("main.rs");
    std::fs::write(&path, "one\ntwo\n").unwrap();
    let (view, cx) = edit(&path, cx);
    cx.simulate_input("mine ");
    std::fs::write(&path, "theirs\n").unwrap();
    cx.simulate_keystrokes("cmd-s");
    cx.run_until_parked();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "theirs\n");
    let id = view.read_with(cx, |view, _| *view.code_views.keys().next().unwrap());
    view.read_with(cx, |view, cx| {
        assert!(view.code_editor(id).unwrap().read(cx).conflicted());
    });
    // Unsaved edits keep Escape from dropping them.
    cx.simulate_keystrokes("escape");
    view.read_with(cx, |view, _| assert!(view.code_editor(id).is_some()));
    draw(cx);
    assert!(cx.debug_bounds("code-edit-overwrite").is_some());
    cx.update(|_, cx| {
        let editor = view.read(cx).code_editor(id).unwrap().clone();
        editor.update(cx, |editor, cx| editor.save(true, cx));
    });
    cx.run_until_parked();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\nmine two\n");
}
