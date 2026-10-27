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
pub(super) fn fixture(
    cx: &mut gpui::TestAppContext,
) -> (Entity<HerdrWindow>, &mut VisualTestContext) {
    let (fixture, cx) = cx.add_window_view(|window, cx| {
        crate::bind_keys(cx);
        let view = cx.new(|cx| {
            let mut view = fixture_window(window, cx);
            view.config.sidebar_style.activity_bar = true;
            let mut shown = snapshot(2);
            shown.focused_workspace_id = Some("w0".into());
            shown.agents[1].agent_status = AgentStatus::Blocked;
            // A folder each: workspaces sharing one share a card.
            for workspace in &mut shown.workspaces {
                workspace.new_workspace_cwd = format!("/tmp/{}", workspace.workspace_id);
            }
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

pub(super) fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        full_draw(window, cx).clear(cx);
    });
}

pub(super) fn bounds(cx: &mut VisualTestContext, selector: &str) -> Option<Bounds<Pixels>> {
    let selector: &'static str = Box::leak(selector.to_owned().into_boxed_str());
    cx.debug_bounds(selector)
}

pub(super) fn click(cx: &mut VisualTestContext, selector: &str) {
    let bounds = bounds(cx, selector).unwrap_or_else(|| panic!("{selector}"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    draw(cx);
}

pub(super) fn endpoint(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| view.read(cx).endpoints[0].id.clone())
}

#[gpui::test]
fn every_workspace_is_listed_in_sidebar_order_and_the_first_unfolds(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    let first = bounds(cx, &format!("folder-{host}-w0")).expect("first row");
    // w1 waits on the user, which no longer moves it up.
    let waiting = bounds(cx, &format!("folder-{host}-w1")).expect("waiting row");
    assert!(first.origin.y < waiting.origin.y, "{first:?} {waiting:?}");
    // The first folder is unfolded, its parts under its row, its Home shown.
    let parts = bounds(cx, "folder-part-terminals").expect("parts");
    assert!(parts.origin.y > first.origin.y && parts.origin.y < waiting.origin.y);
    assert!(
        bounds(cx, "folder-part-home").is_none(),
        "its row is its Home"
    );
    assert!(bounds(cx, "folder-home").is_some());
    assert!(cx.debug_bounds("activity-folders").is_some());
}

#[gpui::test]
fn picking_a_folder_unfolds_it_and_shows_its_home(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    click(cx, &format!("folder-{host}-w0"));
    let picked = bounds(cx, &format!("folder-{host}-w0")).expect("row");
    let parts = bounds(cx, "folder-part-terminals").expect("parts");
    assert!(
        parts.origin.y > picked.origin.y,
        "the parts hang under the picked row"
    );
    assert!(bounds(cx, "folder-home-agents").is_some());
}

#[gpui::test]
fn the_changes_part_fills_the_body_and_home_comes_back(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    click(cx, "folder-part-changes");
    assert!(bounds(cx, "folder-changes").is_some());
    assert!(bounds(cx, "folder-home").is_none());
    // The open folder's own row brings its Home back.
    click(cx, &format!("folder-{host}-w0"));
    assert!(bounds(cx, "folder-home").is_some());
}
