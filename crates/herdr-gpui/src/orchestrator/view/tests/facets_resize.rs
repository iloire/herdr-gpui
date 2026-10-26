//! Dragging the Issues filters' edge resizes the column, and the list takes
//! the room it leaves; a double-click goes back to the default.
use super::*;
use crate::orchestrator::view::rows::Tab;
use gpui::{Modifiers, MouseButton, point, px};

#[gpui::test]
fn the_filters_column_resizes_and_the_list_follows(cx: &mut gpui::TestAppContext) {
    let (_dir, view, cx) = empty::open(cx);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.tab = Tab::Issues;
            view.filters_open = true;
            cx.notify();
        })
    });
    cx.run_until_parked();
    let facets = cx.debug_bounds("orchestrator-facets").unwrap();
    assert_eq!(facets.size.width, px(200.));
    let list = cx.debug_bounds("orchestrator-list").unwrap().left();
    let start = cx
        .debug_bounds("orchestrator-facets-resize")
        .unwrap()
        .center();
    let to = point(start.x + px(70.), start.y);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        point(start.x + px(20.), start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    let wider = cx.debug_bounds("orchestrator-facets").unwrap();
    assert!((wider.size.width - px(270.)).abs() <= px(4.), "{wider:?}");
    let moved = cx.debug_bounds("orchestrator-list").unwrap().left();
    assert!(
        (moved - list - px(70.)).abs() <= px(4.),
        "{list:?} {moved:?}"
    );

    let edge = cx
        .debug_bounds("orchestrator-facets-resize")
        .unwrap()
        .center();
    cx.simulate_event(gpui::MouseDownEvent {
        button: MouseButton::Left,
        position: edge,
        modifiers: Modifiers::default(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_mouse_up(edge, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    assert_eq!(
        cx.debug_bounds("orchestrator-facets").unwrap().size.width,
        px(200.)
    );
}
