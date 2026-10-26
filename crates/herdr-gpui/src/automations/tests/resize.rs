//! Dragging the Automations list's edge, beside an open script, resizes it
//! and gives the page the room it leaves.
use super::section::{click, draw, shown};
use gpui::{Modifiers, MouseButton, point, px};

#[gpui::test]
fn the_list_beside_a_script_resizes(cx: &mut gpui::TestAppContext) {
    let (view, cx) = shown(cx);
    click(cx, "automations-script-0-0");
    let list = cx.debug_bounds("automations-list").unwrap();
    let start = cx.debug_bounds("automations-list-resize").unwrap().center();
    let to = point(start.x + px(60.), start.y);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        point(start.x + px(20.), start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::default());
    draw(cx);
    let wider = cx.debug_bounds("automations-list").unwrap();
    assert!(
        (wider.size.width - list.size.width - px(60.)).abs() <= px(4.),
        "{list:?} {wider:?}"
    );
    let page = cx.debug_bounds("automations-page").unwrap();
    assert!(page.left() >= wider.right());
    view.update(cx, |view, _| {
        assert!(view.automations_list_width.chosen().is_some())
    });
}
