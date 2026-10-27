use super::*;

/// A host whose workspaces hold tabs labelled `(workspace, label)`.
fn with_tabs(workspaces: &[Spec], tabs: &[(&str, &str)]) -> ClientShellSnapshot {
    let mut snapshot = snapshot(workspaces);
    snapshot.tabs = tabs
        .iter()
        .enumerate()
        .map(|(n, (workspace, label))| {
            serde_json::from_value(serde_json::json!({
                "tab_id": format!("t{n}"), "workspace_id": workspace, "number": n + 1,
                "label": label, "custom_label": true, "focused": false,
                "agent_status": "idle", "zoomed": false
            }))
            .unwrap()
        })
        .collect();
    snapshot
}

fn local(snapshot: &ClientShellSnapshot) -> Vec<Card> {
    cards(
        [Host {
            index: 0,
            id: "local",
            label: None,
            snapshot,
            repos: None,
        }],
        &[],
    )
}

/// A tab marked pending shows on its folder's card and keeps the row from
/// fading, counted per workspace and summed across a shared folder.
#[test]
fn a_pending_tab_marks_its_folder() {
    let marked = format!("{}reply to review", crate::pending_tab::MARK);
    let host = with_tabs(
        &[
            ("w1", "app", "/code/app", "main", (0, 0)),
            ("w2", "loops", "/code/app", "main", (0, 0)),
            ("w3", "notes", "/code/notes", "main", (0, 0)),
        ],
        &[
            ("w1", &marked),
            ("w1", "shell"),
            ("w2", &marked),
            ("w3", "shell"),
        ],
    );
    let cards = local(&host);
    let pending: Vec<_> = cards
        .iter()
        .map(|card| (card.title(), card.tabs, card.pending, card.quiet()))
        .collect();
    assert_eq!(pending, [("app", 3, 2, false), ("notes", 1, 0, true)]);
    let members: Vec<_> = cards[0].workspaces.iter().map(|m| m.pending).collect();
    assert_eq!(members, [1, 1]);
}
