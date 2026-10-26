use super::page::{bounds, click, draw, endpoint, fixture};
use crate::menu::Page;
use gpui::{Modifiers, MouseButton};

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
    let row = bounds(cx, &format!("folder-{host}-w1")).expect("row");
    cx.simulate_mouse_down(row.center(), MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(row.center(), MouseButton::Right, Modifiers::default());
    draw(cx);
    assert_eq!(
        cx.update(|_, cx| view.read(cx).menu.page),
        Some(Page::Folder)
    );
    click(cx, "folder-menu-0");
    assert_eq!(cx.update(|_, cx| view.read(cx).menu.page), None);
    let copied = cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert_eq!(copied.as_deref(), Some(folder.as_str()));
}
