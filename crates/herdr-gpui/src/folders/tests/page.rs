use crate::{
    HerdrWindow,
    sidebar::{
        ActivityPanel,
        layout_tests::{SidebarFixture, fixture_window, full_draw, snapshot},
    },
    state::ConnectionStatus,
};
use gpui::{AppContext as _, Bounds, Entity, Modifiers, Pixels, VisualTestContext, px, size};
use herdr_client::protocol::AgentStatus;
use std::sync::Arc;

/// A connected window on Folders: workspace w1's agent waits on the user,
/// w0's works.
fn fixture(cx: &mut gpui::TestAppContext) -> (Entity<HerdrWindow>, &mut VisualTestContext) {
    let (fixture, cx) = cx.add_window_view(|window, cx| {
        crate::bind_keys(cx);
        let view = cx.new(|cx| {
            let mut view = fixture_window(window, cx);
            view.config.sidebar_style.activity_bar = true;
            let mut shown = snapshot(2);
            shown.focused_workspace_id = Some("w0".into());
            shown.agents[1].agent_status = AgentStatus::Blocked;
            view.live.snapshot = Some(Arc::new(shown));
            view.live.status = ConnectionStatus::Connected;
            view
        });
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        SidebarFixture(view)
    });
    cx.simulate_resize(size(px(1200.), px(800.)));
    let view = cx.update(|_, cx| fixture.read(cx).0.clone());
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.show_activity_panel(ActivityPanel::Folders, window, cx);
            view.poll_overview(cx);
        })
    });
    draw(cx);
    (view, cx)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        full_draw(window, cx).clear(cx);
    });
}

fn bounds(cx: &mut VisualTestContext, selector: &str) -> Option<Bounds<Pixels>> {
    let selector: &'static str = Box::leak(selector.to_owned().into_boxed_str());
    cx.debug_bounds(selector)
}

fn click(cx: &mut VisualTestContext, selector: &str) {
    let bounds = bounds(cx, selector).unwrap_or_else(|| panic!("{selector}"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    draw(cx);
}

fn endpoint(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| view.read(cx).endpoints[0].id.clone())
}

#[gpui::test]
fn every_workspace_is_a_card_and_the_waiting_one_leads(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    assert!(bounds(cx, "folders-summary").is_some());
    let waiting = bounds(cx, &format!("folder-{host}-w1")).expect("waiting card");
    let working = bounds(cx, &format!("folder-{host}-w0")).expect("working card");
    // Side by side in one row, or wrapped below: either way it comes first.
    assert!(
        (waiting.origin.y, waiting.origin.x) < (working.origin.y, working.origin.x),
        "{waiting:?} {working:?}"
    );
    // The bar lists the section with the others.
    assert!(cx.debug_bounds("activity-folders").is_some());
}

#[gpui::test]
fn every_card_takes_one_width_and_a_row_never_overflows(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    let page = bounds(cx, "folders-cards").expect("cards");
    let first = bounds(cx, &format!("folder-{host}-w1")).expect("first card");
    let second = bounds(cx, &format!("folder-{host}-w0")).expect("second card");
    assert_eq!(first.size.width, second.size.width);
    assert!(first.size.width >= px(300.));
    assert!(second.right() <= page.right(), "{second:?} past {page:?}");
}

#[gpui::test]
fn needs_me_keeps_only_cards_waiting_on_the_user(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    click(cx, "folders-needs-me");
    assert!(bounds(cx, &format!("folder-{host}-w1")).is_some());
    assert!(bounds(cx, &format!("folder-{host}-w0")).is_none());
    click(cx, "folders-all");
    assert!(bounds(cx, &format!("folder-{host}-w0")).is_some());
}

#[gpui::test]
fn clicking_a_card_opens_its_workspace_with_its_terminals(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    click(cx, &format!("folder-{host}-w1"));
    cx.update(|_, cx| {
        let view = view.read(cx);
        assert_eq!(view.activity_section(), None, "the terminals are shown");
        assert_eq!(view.activity.return_to, Some(ActivityPanel::Folders));
    });
    assert!(bounds(cx, "folders").is_none());
}
