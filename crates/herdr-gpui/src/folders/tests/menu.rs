use super::page::{bounds, click, draw, endpoint, fixture};
use crate::{HerdrWindow, folders::menu::Action, menu::Page};
use gpui::{Entity, Modifiers, MouseButton, VisualTestContext};

/// Right-clicks the element drawn as `selector`.
fn right_click(cx: &mut VisualTestContext, selector: &str) {
    let row = bounds(cx, selector).expect("row");
    cx.simulate_mouse_down(row.center(), MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(row.center(), MouseButton::Right, Modifiers::default());
    draw(cx);
}

/// The open folder menu's actions, in order.
fn actions(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) -> Vec<Action> {
    cx.update(|_, cx| {
        let view = view.read(cx);
        assert_eq!(view.menu.page, Some(Page::Folder));
        view.menu.folder.as_ref().expect("menu").actions()
    })
}

#[gpui::test]
fn right_clicking_a_folder_row_copies_its_path(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    let folder = cx.update(|_, cx| {
        let view = view.read(cx);
        let snapshot = view.live.snapshot.as_deref().unwrap();
        snapshot.workspaces[1].new_workspace_cwd.clone()
    });
    assert!(!folder.is_empty());
    right_click(cx, &format!("folder-{host}-w1"));
    let copy = actions(&view, cx)
        .iter()
        .position(|action| *action == Action::CopyPath)
        .expect("Copy Path");
    click(cx, &format!("folder-menu-{copy}"));
    assert_eq!(cx.update(|_, cx| view.read(cx).menu.page), None);
    let copied = cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert_eq!(copied.as_deref(), Some(folder.as_str()));
}

#[gpui::test]
fn right_clicking_a_folders_files_row_offers_vs_code(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    click(cx, "folder-part-terminals");
    right_click(cx, "folder-part-files");
    let actions = actions(&view, cx);
    assert_eq!(actions.first(), Some(&Action::OpenInVsCode));
    assert!(actions.contains(&Action::CopyPath));
    assert_eq!(
        cx.update(|_, cx| view.read(cx).folders.part),
        crate::folders::Part::Terminals,
        "the right click opens a menu, not the part"
    );
}
