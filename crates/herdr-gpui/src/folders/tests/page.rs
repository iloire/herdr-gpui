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
fn every_workspace_is_listed_and_the_waiting_one_leads_unfolded(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    let waiting = bounds(cx, &format!("folder-{host}-w1")).expect("waiting row");
    let working = bounds(cx, &format!("folder-{host}-w0")).expect("working row");
    assert!(
        waiting.origin.y < working.origin.y,
        "{waiting:?} {working:?}"
    );
    // The first folder is unfolded, its parts under its row, its Home shown.
    let home = bounds(cx, "folder-part-home").expect("parts");
    assert!(home.origin.y > waiting.origin.y && home.origin.y < working.origin.y);
    assert!(bounds(cx, "folder-home").is_some());
    assert!(cx.debug_bounds("activity-folders").is_some());
}

#[gpui::test]
fn picking_a_folder_unfolds_it_and_shows_its_home(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    click(cx, &format!("folder-{host}-w0"));
    let picked = bounds(cx, &format!("folder-{host}-w0")).expect("row");
    let home = bounds(cx, "folder-part-home").expect("parts");
    assert!(
        home.origin.y > picked.origin.y,
        "the parts hang under the picked row"
    );
    assert!(bounds(cx, "folder-home-agents").is_some());
}

#[gpui::test]
fn the_changes_part_fills_the_body_and_home_comes_back(cx: &mut gpui::TestAppContext) {
    let (_, cx) = fixture(cx);
    click(cx, "folder-part-changes");
    assert!(bounds(cx, "folder-changes").is_some());
    assert!(bounds(cx, "folder-home").is_none());
    click(cx, "folder-part-home");
    assert!(bounds(cx, "folder-home").is_some());
}

#[gpui::test]
fn needs_me_keeps_only_folders_waiting_on_the_user(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    click(cx, "folders-needs-me");
    assert!(bounds(cx, &format!("folder-{host}-w1")).is_some());
    assert!(bounds(cx, &format!("folder-{host}-w0")).is_none());
    click(cx, "folders-all");
    assert!(bounds(cx, &format!("folder-{host}-w0")).is_some());
}

#[gpui::test]
fn terminals_opens_the_folders_workspace(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    click(cx, "folder-part-terminals");
    cx.update(|_, cx| {
        let view = view.read(cx);
        assert_eq!(view.activity_section(), None, "the terminals are shown");
        assert_eq!(view.activity.return_to, Some(ActivityPanel::Folders));
    });
    assert!(bounds(cx, "folders").is_none());
}
