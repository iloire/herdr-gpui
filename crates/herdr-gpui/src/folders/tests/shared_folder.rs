use super::page::{bounds, click, draw, endpoint, fixture};
use super::{Card, Host, State, cards, row, snapshot};
use crate::folders::Part;
use std::sync::Arc;

fn local<'a>(shown: &'a herdr_client::protocol::ClientShellSnapshot) -> Host<'a> {
    Host {
        index: 0,
        id: "local",
        label: None,
        snapshot: shown,
        repos: None,
    }
}

fn members(card: &Card) -> Vec<&str> {
    card.workspaces
        .iter()
        .map(|member| member.id.as_str())
        .collect()
}

/// Workspaces open in one folder share its card, where the first of them
/// sits, with the most pressing of their agents; each keeps its own.
#[test]
fn workspaces_in_one_folder_share_a_card() {
    let shown = snapshot(&[
        ("w1", "notes", "/me/notes", "main", (0, 0)),
        ("w2", "app", "/me/app", "main", (0, 0)),
        ("w3", "loops", "/me/notes", "main", (0, 0)),
    ]);
    let rows = [
        row("local", "w1", State::Working, "Writing", 1),
        row("local", "w3", State::Blocked, "Should I continue?", 1),
    ];
    let cards = cards([local(&shown)], &rows);
    assert_eq!(cards.len(), 2);
    let notes = &cards[0];
    assert_eq!(members(notes), ["w1", "w3"]);
    assert_eq!(notes.workspace, "w1", "the first workspace stands for it");
    assert!(
        notes.waiting(),
        "the waiting agent in loops marks the folder"
    );
    assert_eq!(
        notes.alias(),
        None,
        "each workspace is named on its own row"
    );
    let states: Vec<_> = notes
        .workspaces
        .iter()
        .map(|member| member.agent.as_ref().map(|agent| agent.state))
        .collect();
    assert_eq!(states, [Some(State::Working), Some(State::Blocked)]);
    assert_eq!(members(&cards[1]), ["w2"]);
}

/// The same path on two hosts is two folders, and workspaces the daemon
/// names no folder for never merge.
#[test]
fn only_one_hosts_named_folder_merges() {
    let here = snapshot(&[
        ("w1", "api", "/srv/api", "main", (0, 0)),
        ("w2", "scratch", "", "main", (0, 0)),
        ("w3", "other", "", "main", (0, 0)),
    ]);
    let there = snapshot(&[("w1", "api", "/srv/api", "main", (0, 0))]);
    let remote = Host {
        index: 1,
        id: "nas",
        label: Some("nas"),
        snapshot: &there,
        repos: None,
    };
    let cards = cards([local(&here), remote], &[]);
    assert_eq!(cards.len(), 4);
    assert!(cards.iter().all(|card| card.workspaces.len() == 1));
}

/// A shared folder lists one row, and a Terminals row for each workspace
/// that switches to that workspace.
#[gpui::test]
fn a_shared_folder_lists_a_terminals_row_per_workspace(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            if let Some(snapshot) = view.live.snapshot.as_deref() {
                let mut shown = snapshot.clone();
                shown.workspaces[1].new_workspace_cwd =
                    shown.workspaces[0].new_workspace_cwd.clone();
                view.live.snapshot = Some(Arc::new(shown));
            }
            cx.notify();
        })
    });
    draw(cx);
    assert!(bounds(cx, &format!("folder-{host}-w0")).is_some());
    assert!(bounds(cx, &format!("folder-{host}-w1")).is_none(), "merged");
    assert!(bounds(cx, "folder-part-terminals").is_none());
    assert!(bounds(cx, "folder-part-terminals-w0").is_some());
    click(cx, "folder-part-terminals-w1");
    cx.update(|_, cx| {
        let view = view.read(cx);
        assert_eq!(view.folders.part, Part::Terminals);
        assert_eq!(view.folders.selected, Some((host.clone(), "w1".into())));
    });
    assert!(
        bounds(cx, "folder-part-terminals-w1").is_some(),
        "the folder stays unfolded on its second workspace"
    );
}
