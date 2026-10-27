use super::*;

/// One local folder, `ahead` commits to push, with an agent per `states`.
fn card(ahead: usize, states: &[State]) -> Card {
    let local = snapshot(&[("w1", "app", "/code/app", "main", (ahead, 0))]);
    let rows: Vec<Row> = states
        .iter()
        .map(|&state| row("local", "w1", state, "Task", 1))
        .collect();
    cards(
        [Host {
            index: 0,
            id: "local",
            label: None,
            snapshot: &local,
            repos: None,
        }],
        &rows,
    )
    .remove(0)
}

/// A folder is quiet, and its row dimmed, only when its agent has nothing
/// new to show and nothing waits to be committed or pushed.
#[test]
fn a_folder_is_quiet_only_when_nothing_asks_for_a_look() {
    assert!(card(0, &[]).quiet());
    assert!(card(0, &[State::Idle]).quiet());
    for state in [State::Blocked, State::Failed, State::Working, State::Done] {
        assert!(!card(0, &[state]).quiet(), "{state:?}");
    }
    assert!(!card(1, &[]).quiet());
    let mut dirty = card(0, &[]);
    dirty.dirty = Some(Dirty {
        files: 2,
        ..Dirty::default()
    });
    assert!(!dirty.quiet());
    dirty.dirty = Some(Dirty::default());
    assert!(dirty.quiet());
}
