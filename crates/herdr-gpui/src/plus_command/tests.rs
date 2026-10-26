//! The "+" that runs `plus_command`: the setting, its mark, the tab it
//! opens and the line typed there, and Alt turning it back into a plain "+".

use super::*;
use crate::{
    sidebar::layout_tests::fixture_window, window::MockPeer, worktree_scripts::tests::connect,
};
use herdr_client::protocol::{ClientMessage as DaemonMessage, ClientPaneInputEvent};

fn command(line: &str) -> PlusCommand {
    PlusCommand::try_from(line.to_owned()).unwrap()
}

#[test]
fn a_command_is_one_trimmed_line() {
    assert_eq!(
        command("  claude --continue ").as_str(),
        "claude --continue"
    );
    for refused in ["", "   ", "claude\nrm -rf ~", "claude\u{1b}[A"] {
        assert!(
            PlusCommand::try_from(refused.to_owned()).is_err(),
            "{refused:?}"
        );
    }
    assert!(PlusCommand::try_from("x".repeat(MAX_COMMAND_BYTES + 1)).is_err());
}

#[test]
fn the_mark_follows_the_program_the_line_runs() {
    assert_eq!(command("claude").icon(), AgentIcon::Claude);
    assert_eq!(
        command("/opt/homebrew/bin/codex resume --last").program(),
        "codex"
    );
    assert_eq!(command("C:\\Tools\\claude.cmd").program(), "claude.cmd");
    assert_eq!(command("htop").icon(), AgentIcon::Generic);
}

#[gpui::test]
fn alt_turns_the_command_back_into_a_plain_tab(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|_, cx| {
        view.update(cx, |view, _| {
            view.active = true;
            let alt = Modifiers {
                alt: true,
                ..Modifiers::default()
            };
            // Without the setting there is nothing to redraw.
            assert!(!view.track_plus_alt(&alt));
            assert!(view.plus_command().is_none());
            view.config.plus_command = Some(command("claude"));
            assert!(!view.track_plus_alt(&alt), "Alt was already down");
            assert!(view.plus_command().is_none());
            assert!(view.track_plus_alt(&Modifiers::default()));
            assert_eq!(view.plus_command(), Some(&command("claude")));
            // Alt held as the window lost focus does not stick.
            view.track_plus_alt(&alt);
            view.active = false;
            assert_eq!(view.plus_command(), Some(&command("claude")));
        })
    });
}

#[gpui::test]
fn plus_opens_a_tab_and_types_the_command(cx: &mut gpui::TestAppContext) {
    let mut peer = MockPeer::advertising(&["tab.create"]);
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            connect(view, &peer, cx);
            view.start_plus_command(&command("claude --continue"))
                .unwrap();
            // One tab opens at a time.
            assert!(view.script_slot_busy());
            assert!(matches!(
                view.start_plus_command(&command("codex")),
                Err(Error::SlotBusy)
            ));
        })
    });
    let tab = peer.request();
    assert_eq!(tab["method"], "tab.create");
    assert_eq!(
        tab["params"],
        json!({"workspace_id": "w1", "label": "claude", "focus": true, "cwd": "/repo"})
    );
    let created = json!({"result":{"type":"tab_created",
        "tab":{"tab_id":"w1:t9"},"root_pane":{"pane_id":"w1:p9"}}});
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            let id = tab["id"].as_str().unwrap().to_owned();
            view.live.script_response = Some((id, Some(Ok(created))));
            view.poll_plus_command(cx);
            assert!(!view.plus_command_starting());
            assert!(view.flash.is_none(), "{:?}", view.flash);
        })
    });
    let typed = std::iter::repeat_with(|| peer.receive())
        .take(8)
        .find(|message| matches!(message, DaemonMessage::ClientShellPaneInput { .. }));
    assert_eq!(
        typed.unwrap(),
        DaemonMessage::ClientShellPaneInput {
            pane_id: "w1:p9".into(),
            events: vec![
                ClientPaneInputEvent::TextCommit("claude --continue".into()),
                crate::menu::enter_key(),
            ],
        }
    );
}

#[gpui::test]
fn a_start_from_a_replaced_connection_types_nothing(cx: &mut gpui::TestAppContext) {
    let peer = MockPeer::advertising(&["tab.create"]);
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            connect(view, &peer, cx);
            view.start_plus_command(&command("claude")).unwrap();
            view.endpoints[0].generation += 1;
            view.poll_plus_command(cx);
            assert!(!view.plus_command_starting());
            assert!(view.flash.is_some(), "the user is told it did not start");
        })
    });
}
