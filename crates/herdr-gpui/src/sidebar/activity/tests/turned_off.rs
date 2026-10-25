//! `[sidebar] pull_requests` and `issues = false`: the bar leaves that
//! section off, and a window left on it shows Workspaces.
use super::sections::{local, show};
use super::*;
use crate::sidebar::layout_tests::full_draw;

#[gpui::test]
fn a_turned_off_section_leaves_the_bar_and_cannot_be_opened(cx: &mut gpui::TestAppContext) {
    let (view, cx) = local(cx);
    let draw = |cx: &mut gpui::VisualTestContext| {
        cx.update(|window, cx| full_draw(window, cx).clear(cx));
    };
    for (panel, id) in [
        (ActivityPanel::PullRequests, "activity-pull-requests"),
        (ActivityPanel::Issues, "activity-issues"),
    ] {
        show(&view, cx, panel);
        assert!(cx.debug_bounds(id).is_some());
        assert!(cx.debug_bounds("activity-section").is_some());

        let set = |cx: &mut gpui::VisualTestContext, shown: bool| {
            cx.update(|_, cx| {
                view.update(cx, |view, _| {
                    let style = &mut view.config.sidebar_style;
                    match panel {
                        ActivityPanel::PullRequests => style.pull_requests = shown,
                        _ => style.issues = shown,
                    }
                })
            });
        };
        set(cx, false);
        draw(cx);
        assert!(cx.debug_bounds(id).is_none(), "{id}");
        assert!(cx.debug_bounds("activity-section").is_none());
        assert!(cx.debug_bounds("activity-workspaces").is_some());
        cx.update(|_, cx| assert_eq!(view.read(cx).activity_section(), None));

        // Asking for it again is ignored while it is off.
        show(&view, cx, panel);
        assert!(cx.debug_bounds("activity-section").is_none());

        // Back on, the window returns to the section it was left on.
        set(cx, true);
        draw(cx);
        assert!(cx.debug_bounds(id).is_some());
        assert!(cx.debug_bounds("activity-section").is_some());
        show(&view, cx, ActivityPanel::Workspaces);
    }
}
