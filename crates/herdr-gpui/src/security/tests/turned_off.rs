//! `[sidebar] security = false`: the section reads nothing from GitHub.
use super::*;

#[gpui::test]
fn a_turned_off_section_starts_no_read(cx: &mut gpui::TestAppContext) {
    let (view, cx) = section::window(cx);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.config.sidebar_style.security = false;
            view.poll_security(cx);
            assert!(!view.security.busy());
            assert!(view.security.next.is_none(), "nothing is scheduled");

            // Back on, the next tick reads, or waits for something to read.
            view.config.sidebar_style.security = true;
            view.poll_security(cx);
            assert!(view.security.busy() || view.security.next.is_some());
        });
    });
}
