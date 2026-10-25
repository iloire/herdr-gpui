//! Files is a column beside the terminals, as Workspaces is, rather than a
//! section across the window's body.
use super::sections::local;
use super::*;
use crate::sidebar::layout_tests::full_draw;
use gpui::{Modifiers, VisualTestContext};

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx.debug_bounds(selector).unwrap();
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.update(|window, cx| {
        window.refresh();
        full_draw(window, cx).clear(cx);
    });
}

#[gpui::test]
fn files_takes_the_sidebars_place_and_keeps_the_terminals(cx: &mut gpui::TestAppContext) {
    let (view, cx) = local(cx);
    click(cx, "activity-files");
    let bar = cx.debug_bounds("activity-bar").unwrap();
    let files = cx.debug_bounds("files").unwrap();
    assert_eq!(files.left(), bar.right());
    assert!(cx.debug_bounds("sidebar").is_none());
    assert!(cx.debug_bounds("activity-section").is_none());
    cx.update(|_, cx| {
        let view = view.read(cx);
        assert_eq!(view.activity.panel, ActivityPanel::Files);
        assert_eq!(view.activity_section(), None);
        assert!(view.files_shown());
        let width = view.sidebar_column_width(1000.).unwrap();
        assert_eq!(width, BAR_WIDTH + f32::from(files.size.width));
    });
}

#[gpui::test]
fn picking_files_again_folds_the_column_and_a_third_time_brings_it_back(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = local(cx);
    click(cx, "activity-files");
    click(cx, "activity-files");
    assert!(cx.debug_bounds("files").is_none());
    assert!(!cx.update(|_, cx| view.read(cx).files_shown()));
    click(cx, "activity-files");
    assert!(cx.debug_bounds("files").is_some());
}

#[gpui::test]
fn another_section_hides_the_tree_and_stops_reading_it(cx: &mut gpui::TestAppContext) {
    let (view, cx) = local(cx);
    click(cx, "activity-files");
    click(cx, "activity-changes");
    assert!(cx.debug_bounds("files").is_none());
    assert!(!cx.update(|_, cx| view.read(cx).files_shown()));
    click(cx, "activity-workspaces");
    assert!(cx.debug_bounds("sidebar").is_some());
}
