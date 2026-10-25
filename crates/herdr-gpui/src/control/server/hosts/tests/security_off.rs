//! `security` and `security open` refuse while the section is turned off,
//! rather than answer from a read that is no longer kept up.
use super::*;
use crate::control::server::hosts::security::{alerts, open};
use crate::sidebar::ActivityPanel;

#[gpui::test]
fn both_commands_refuse_while_security_is_off(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    cx.update(|_, cx| view.update(cx, |view, _| view.config.sidebar_style.security = false));
    let listing = view.read_with(cx, |view, _| alerts(view));
    assert_eq!(error_code(&listing), Some(ErrorCode::Unsupported));
    let Response::Error { message, .. } = &listing else {
        panic!("{listing:?}");
    };
    assert!(message.contains("[sidebar] security = false"), "{message}");

    let opened = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            open(view, "penso/herdr-gpui", "42", window, cx)
        })
    });
    assert_eq!(error_code(&opened), Some(ErrorCode::Unsupported));
    let panel = view.read_with(cx, |view, _| view.activity.panel);
    assert_ne!(panel, ActivityPanel::Security);
}
