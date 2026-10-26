//! Dragging the Folders list's edge, and the file tree's beside it, resizes
//! them and gives the terminals the room they leave; a double-click goes
//! back to the default.
use super::page::{bounds, click, draw, fixture};
use gpui::{Modifiers, MouseButton, Pixels, VisualTestContext, point, px};

pub(crate) fn drag(cx: &mut VisualTestContext, handle: &str, by: Pixels) {
    let start = bounds(cx, handle).unwrap().center();
    let to = point(start.x + by, start.y);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        point(start.x + by / 4., start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::default());
    draw(cx);
}

fn width(cx: &mut VisualTestContext, selector: &str) -> Pixels {
    bounds(cx, selector).unwrap().size.width
}

#[gpui::test]
fn the_list_and_tree_resize_and_the_terminals_take_the_room(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    assert_eq!(width(cx, "folders-list"), px(290.));
    // On the page, the list narrows.
    drag(cx, "folders-list-resize", px(-80.));
    assert!((width(cx, "folders-list") - px(210.)).abs() <= px(4.));
    // Beside the terminals it keeps that width, and the tree resizes too.
    click(cx, "folder-part-files");
    assert!((width(cx, "folders-list") - px(210.)).abs() <= px(4.));
    let terminal = bounds(cx, "stand-in").unwrap().left();
    drag(cx, "folders-tree-resize", px(-60.));
    assert!((width(cx, "files") - px(200.)).abs() <= px(4.));
    let wider = bounds(cx, "stand-in").unwrap().left();
    assert!(
        (terminal - wider - px(60.)).abs() <= px(6.),
        "the terminals take the room: {terminal:?} {wider:?}"
    );
    view.update(cx, |view, _| {
        assert!(view.folders.list_width.chosen().is_some());
        assert!(view.files_tree_width.chosen().is_some());
        assert!(!view.folders.list_width.take_unsaved(), "saved on release");
    });
    // A double-click on the list's edge goes back to the default.
    let edge = bounds(cx, "folders-list-resize").unwrap().center();
    cx.simulate_event(gpui::MouseDownEvent {
        button: MouseButton::Left,
        position: edge,
        modifiers: Modifiers::default(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_mouse_up(edge, MouseButton::Left, Modifiers::default());
    draw(cx);
    assert_eq!(width(cx, "folders-list"), px(290.));
}

#[gpui::test]
fn the_changes_part_list_resizes(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    click(cx, "folder-part-changes");
    let before = width(cx, "folder-changes-panel");
    drag(cx, "folder-changes-resize", px(-100.));
    let after = width(cx, "folder-changes-panel");
    assert!(
        (before - after - px(100.)).abs() <= px(4.),
        "{before:?} {after:?}"
    );
    view.update(cx, |view, _| {
        assert!(view.changes_list_width.chosen().is_some())
    });
}
