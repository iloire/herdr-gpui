//! Dragging the Changes list's right edge resizes it, giving the review
//! beside it the room it leaves; a double-click goes back to the default.
use super::*;
use gpui::{Modifiers, MouseButton, point, px, size};

fn draw(cx: &mut gpui::VisualTestContext) {
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
}

#[gpui::test]
fn dragging_the_list_edge_resizes_it_and_a_double_click_resets(cx: &mut gpui::TestAppContext) {
    let (view, cx) = window(cx, Read::default());
    cx.simulate_resize(size(px(1600.), px(900.)));
    draw(cx);
    let list = cx.debug_bounds("changes-list-panel").unwrap();
    assert_eq!(list.size.width, px(420.));
    let edge = cx.debug_bounds("changes-list-resize").unwrap();
    assert!(
        (edge.right() - list.right()).abs() <= px(1.),
        "on the edge facing the review: {edge:?} {list:?}"
    );

    // Dragging the edge 150px left narrows the list by as much.
    let start = edge.center();
    let to = point(start.x - px(150.), start.y);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        point(start.x - px(40.), start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::default());
    draw(cx);
    let narrower = cx.debug_bounds("changes-list-panel").unwrap();
    assert!(
        (narrower.size.width - px(270.)).abs() <= px(4.),
        "{narrower:?}"
    );
    assert_eq!(narrower.left(), list.left(), "the far edge stays put");
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::default());
    view.update(cx, |view, _| {
        assert!(view.changes_list_width.chosen().is_some());
        assert!(!view.changes_list_width.take_unsaved(), "saved on release");
    });

    // A double-click on the edge goes back to the default width.
    let edge = cx.debug_bounds("changes-list-resize").unwrap();
    cx.simulate_event(gpui::MouseDownEvent {
        button: MouseButton::Left,
        position: edge.center(),
        modifiers: Modifiers::default(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_mouse_up(edge.center(), MouseButton::Left, Modifiers::default());
    draw(cx);
    assert_eq!(
        cx.debug_bounds("changes-list-panel").unwrap().size.width,
        px(420.)
    );
    view.update(cx, |view, _| {
        assert_eq!(view.changes_list_width.chosen(), None)
    });
}
