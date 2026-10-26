use super::page::{bounds, draw, endpoint, fixture};
use crate::{reorder::LIFT_DELAY, state::ConnectionStatus, window::MockPeer};
use gpui::{Modifiers, MouseButton, VisualTestContext, point, px};

fn lifted(view: &gpui::Entity<crate::HerdrWindow>, cx: &mut VisualTestContext) -> Option<bool> {
    view.read_with(cx, |view, _| {
        view.folders.drag.as_ref().map(|drag| drag.lifted)
    })
}

/// A held press lifts a folder; the folder it passes closes its place; a
/// release without a daemon to send to puts every row back; a quick click
/// stays a click, and Escape drops a lifted row where it was.
#[gpui::test]
fn dragging_a_folder_previews_its_drop_and_a_click_stays_a_click(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    let (first_id, second_id) = (format!("folder-{host}-w0"), format!("folder-{host}-w1"));
    let first = bounds(cx, &first_id).expect("first");
    let second = bounds(cx, &second_id).expect("second");

    // A quick click opens the folder, with nothing lifted.
    cx.simulate_mouse_down(second.center(), MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(second.center(), MouseButton::Left, Modifiers::default());
    cx.executor().advance_clock(LIFT_DELAY * 2);
    cx.run_until_parked();
    draw(cx);
    assert_eq!(lifted(&view, cx), None);
    let selected = view.read_with(cx, |view, _| view.folders.selected.clone());
    assert_eq!(selected, Some((host.clone(), "w1".into())));
    // Back to the first, so the rows rest as they started.
    cx.simulate_click(first.center(), Modifiers::default());
    draw(cx);
    assert_eq!(bounds(cx, &second_id), Some(second));

    // Held in place, the row lifts without moving.
    cx.simulate_mouse_down(first.center(), MouseButton::Left, Modifiers::default());
    assert_eq!(lifted(&view, cx), Some(false));
    cx.executor().advance_clock(LIFT_DELAY);
    cx.run_until_parked();
    draw(cx);
    assert_eq!(lifted(&view, cx), Some(true));

    // Carried past the second folder's middle, it follows the pointer and
    // the second closes its place.
    let below = point(first.center().x, second.bottom() - px(2.));
    cx.simulate_mouse_move(below, MouseButton::Left, Modifiers::default());
    draw(cx);
    let carried = bounds(cx, &first_id).expect("carried");
    let followed = carried.top() - first.top() - (below.y - first.center().y);
    assert!(followed.abs() < px(0.5), "{carried:?} {first:?}");
    // Within layout rounding.
    let passed = bounds(cx, &second_id).expect("passed").top();
    assert!(
        (passed - first.top()).abs() < px(0.5),
        "{passed:?} {first:?}"
    );
    let slot = view.read_with(cx, |view, _| {
        view.folders.drag.as_ref().and_then(|d| d.slot)
    });
    assert_eq!(slot, Some(2));

    // The release is the drop, not a click; with no daemon nothing moves.
    cx.simulate_mouse_up(below, MouseButton::Left, Modifiers::default());
    draw(cx);
    assert_eq!(lifted(&view, cx), None);
    assert_eq!(bounds(cx, &first_id), Some(first));
    assert_eq!(bounds(cx, &second_id), Some(second));
    let selected = view.read_with(cx, |view, _| view.folders.selected.clone());
    assert_eq!(selected, Some((host.clone(), "w0".into())));

    // Escape drops a lifted row where it came from.
    cx.simulate_mouse_down(first.center(), MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(below, MouseButton::Left, Modifiers::default());
    draw(cx);
    assert_eq!(lifted(&view, cx), Some(true));
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert_eq!(lifted(&view, cx), None);
    assert_eq!(bounds(cx, &first_id), Some(first));
}

/// Dropping a folder sends its move to the host's daemon, which owns the
/// order the sidebar shares.
#[gpui::test]
fn dropping_a_folder_sends_its_move_to_the_daemon(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let mut peer = MockPeer::advertising(&["workspace.move_block"]);
    cx.update(|_, cx| {
        view.update(cx, |view, _| {
            // The mock daemon's boot, which a request must name.
            std::sync::Arc::make_mut(view.live.snapshot.as_mut().unwrap()).boot_id =
                "boot-v1".into();
            view.endpoints[0].connection.handle = Some(peer.client.handle.clone());
            view.endpoints[0].live = view.live.clone();
            view.endpoints[0].live.status = ConnectionStatus::Connected;
        })
    });
    let host = endpoint(&view, cx);
    let first = bounds(cx, &format!("folder-{host}-w0")).expect("first");
    let second = bounds(cx, &format!("folder-{host}-w1")).expect("second");
    let below = point(first.center().x, second.bottom() - px(2.));
    cx.simulate_mouse_down(first.center(), MouseButton::Left, Modifiers::default());
    // The first move lifts the row; the next resolves its gap against the
    // lifted frame's layout.
    for _ in 0..2 {
        cx.simulate_mouse_move(below, MouseButton::Left, Modifiers::default());
        draw(cx);
    }
    cx.simulate_mouse_up(below, MouseButton::Left, Modifiers::default());
    draw(cx);
    cx.run_until_parked();
    let request = peer.request();
    assert_eq!(request["method"], "workspace.move_block");
    assert_eq!(
        request["params"],
        serde_json::json!({"workspace_ids": ["w0"], "before_workspace_id": null})
    );
    // The preview holds until the daemon's new order arrives.
    assert_eq!(lifted(&view, cx), Some(true));
}
