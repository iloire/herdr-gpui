//! A folder's agent shows the status mark Spaces shows, in the configured
//! icon set, centered on its row.

use super::page::{bounds, draw, endpoint, fixture};
use crate::config::StatusIcons;
use herdr_client::protocol::AgentStatus;

/// The selector the sidebar's working mark carries in `icons`, when the set
/// draws one with a selector.
fn working_selector(icons: StatusIcons) -> Option<&'static str> {
    if icons == StatusIcons::Dots {
        return Some("status-orbit");
    }
    crate::icons::status_icon(icons, AgentStatus::Working).map(|_| "status-spin")
}

#[gpui::test]
fn a_working_folder_shows_the_sidebars_mark(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    let mut drawn = 0;
    for &icons in StatusIcons::ALL {
        let Some(selector) = working_selector(icons) else {
            continue;
        };
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.config.status_icons = icons;
                cx.notify();
            })
        });
        draw(cx);
        let row = bounds(cx, &format!("folder-{host}-w0")).expect("working row");
        let mark = bounds(cx, selector).unwrap_or_else(|| panic!("{icons:?}: no {selector}"));
        assert!(
            row.contains(&mark.center()),
            "{icons:?}: {selector} {mark:?} is not on {row:?}"
        );
        let offset = (mark.center().y - row.center().y).abs();
        assert!(
            offset <= gpui::px(1.5),
            "{icons:?}: {selector} {mark:?} is off {row:?}'s middle"
        );
        drawn += usize::from(icons != StatusIcons::Dots);
    }
    assert!(drawn > 0, "no drawn icon set was tried");
}
