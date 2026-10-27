#![allow(clippy::unwrap_used)]
use super::*;

fn snapshot(boot: &str, tabs: &[(&str, &str, AgentStatus)]) -> ClientShellSnapshot {
    let mut snapshot: ClientShellSnapshot = serde_json::from_str(include_str!(
        "../../../herdr-protocol/tests/fixtures/endpoint-snapshot-v1.json"
    ))
    .unwrap();
    snapshot.boot_id = boot.into();
    let template = snapshot.tabs[0].clone();
    snapshot.tabs = tabs
        .iter()
        .map(|(id, label, status)| ClientShellTab {
            tab_id: (*id).into(),
            label: (*label).into(),
            agent_status: *status,
            ..template.clone()
        })
        .collect();
    snapshot
}

fn started(watch: &mut Watch, snapshot: &ClientShellSnapshot) -> Vec<String> {
    watch
        .started(snapshot)
        .into_iter()
        .map(|tab| tab.tab_id.clone())
        .collect()
}

#[test]
fn toggling_adds_the_mark_and_takes_it_off() {
    assert_eq!(
        toggle_params("t1", "claude"),
        json!({"tab_id": "t1", "label": "\u{25c6} claude"})
    );
    assert_eq!(
        toggle_params("t1", "\u{25c6} claude"),
        json!({"tab_id": "t1", "label": "claude"})
    );
    // Only the exact prefix is the mark: a diamond in a name is the name.
    assert!(!is_marked("\u{25c6}claude"));
    assert!(!is_marked("fix \u{25c6} claude"));
    assert_eq!(unmarked("\u{25c6} \u{25c6} a"), "\u{25c6} a");
}

#[test]
fn only_a_plain_enter_press_submits() {
    let key = |code, kind, modifiers| ClientPaneInputEvent::Key {
        code,
        modifiers,
        kind,
        repeat_count: 1,
        shifted_codepoint: None,
        generated_text: None,
        tracks_release: false,
        physical_key_id: None,
        windows_record: None,
    };
    assert!(submits(&key(ClientKeyCode::Enter, ClientKeyKind::Press, 0)));
    assert!(!submits(&key(
        ClientKeyCode::Enter,
        ClientKeyKind::Press,
        1
    )));
    assert!(!submits(&key(
        ClientKeyCode::Enter,
        ClientKeyKind::Release,
        0
    )));
    assert!(!submits(&key(ClientKeyCode::Tab, ClientKeyKind::Press, 0)));
    assert!(!submits(&ClientPaneInputEvent::TextCommit("\n".into())));
}

#[test]
fn a_marked_tab_starting_to_work_is_cleared_once() {
    use AgentStatus::{Done, Idle, Working};
    let mut watch = Watch::default();
    let marked = "\u{25c6} claude";
    // First sight only records, even of a working tab.
    assert!(
        started(
            &mut watch,
            &snapshot("b1", &[("t1", marked, Working), ("t2", marked, Idle)])
        )
        .is_empty()
    );
    // Still working: no change, so marking a busy tab keeps its mark.
    assert!(
        started(
            &mut watch,
            &snapshot("b1", &[("t1", marked, Working), ("t2", marked, Idle)])
        )
        .is_empty()
    );
    assert_eq!(
        started(
            &mut watch,
            &snapshot("b1", &[("t1", marked, Done), ("t2", marked, Working)])
        ),
        ["t2"]
    );
    assert_eq!(
        started(
            &mut watch,
            &snapshot("b1", &[("t1", marked, Working), ("t2", marked, Working)])
        ),
        ["t1"]
    );
    // An unmarked tab starting work has nothing to clear.
    assert!(started(&mut watch, &snapshot("b1", &[("t1", "claude", Idle)])).is_empty());
    assert!(started(&mut watch, &snapshot("b1", &[("t1", "claude", Working)])).is_empty());
}

#[test]
fn a_new_boot_records_before_it_clears() {
    use AgentStatus::{Idle, Working};
    let mut watch = Watch::default();
    let marked = "\u{25c6} claude";
    started(&mut watch, &snapshot("b1", &[("t1", marked, Idle)]));
    assert!(started(&mut watch, &snapshot("b2", &[("t1", marked, Working)])).is_empty());
}
