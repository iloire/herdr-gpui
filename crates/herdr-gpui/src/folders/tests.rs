#![allow(clippy::expect_used, clippy::unwrap_used)]

mod drag;
mod edge_bar;
mod new_button;
mod page;
mod parts;
mod resize;
mod status_icons;

use super::model::{Card, Dirty, Host, Summary, cards, hue, monogram};
use crate::{
    changes::{Repo, Shown, Workspace},
    overview::{Key, Open, Row, State},
};
use herdr_client::protocol::ClientShellSnapshot;
use herdr_vcs::changes::{Checkout, FileChange, Kind};
use std::time::{Duration, UNIX_EPOCH};

/// A workspace as `(id, label, folder, branch, ahead/behind)`.
type Spec<'a> = (&'a str, &'a str, &'a str, &'a str, (usize, usize));

/// A host with `workspaces`.
fn snapshot(workspaces: &[Spec]) -> ClientShellSnapshot {
    serde_json::from_value(serde_json::json!({
        "boot_id": "folders-test", "revision": 1,
        "update_install_command": "", "latest_release_notes_available": false,
        "integration_updates_available": false, "worktree_directory": "",
        "tab_bar_right": [], "tab_bar_right_separator": "", "agent_order": [],
        "tabs": [], "panes": [], "commands": [], "agents": [],
        "workspaces": workspaces.iter().enumerate().map(|(number, (id, label, folder, branch, sync))| serde_json::json!({
            "workspace_id": id, "active_tab_id": "t0", "new_workspace_cwd": folder,
            "number": number + 1, "label": label, "custom_label": false,
            "branch": branch, "git_ahead_behind": [sync.0, sync.1],
            "tokens": [], "focused": false, "agent_status": "idle"
        })).collect::<Vec<_>>(),
    }))
    .unwrap()
}

fn row(endpoint: &str, workspace: &str, state: State, title: &str, since: u64) -> Row {
    Row {
        key: Key::Agent {
            endpoint: endpoint.into(),
            checkout: workspace.into(),
            agent: title.into(),
        },
        state,
        title: title.into(),
        detail: String::new(),
        identity: Some("claude".into()),
        workspace: workspace.into(),
        branch: None,
        host: endpoint.into(),
        endpoint: endpoint.into(),
        since: Some(UNIX_EPOCH + Duration::from_secs(since)),
        pr: None,
        open: Open::Pane {
            endpoint: endpoint.into(),
            pane: format!("pane-{title}"),
            tab: "t0".into(),
            workspace: workspace.into(),
        },
    }
}

/// A read checkout at `path` open in `workspaces`, with `files` changed.
fn shown(path: &str, workspaces: &[&str], files: usize, ahead: u32) -> Shown {
    let changed = (0..files)
        .map(|file| FileChange {
            path: format!("f{file}.rs"),
            kind: Kind::Modified,
            additions: Some(3),
            deletions: Some(1),
        })
        .collect();
    Shown {
        checkout: Checkout {
            path: path.into(),
            common_dir: format!("{path}/.git"),
            branch: Some("read-branch".into()),
            ahead,
            files: changed,
            total_files: files,
            ..Checkout::default()
        },
        workspaces: workspaces
            .iter()
            .map(|id| Workspace {
                id: (*id).into(),
                label: (*id).into(),
            })
            .collect(),
    }
}

fn repo(checkouts: Vec<Shown>) -> Repo {
    Repo {
        name: "repo".into(),
        checkouts,
    }
}

fn names(cards: &[Card]) -> Vec<&str> {
    cards.iter().map(|card| card.name.as_str()).collect()
}

#[test]
fn cards_follow_sidebar_order_whatever_needs_the_user() {
    let local = snapshot(&[
        ("w1", "quiet", "/code/quiet", "main", (0, 0)),
        ("w2", "dirty", "/code/dirty", "main", (0, 0)),
        ("w3", "unpushed", "/code/unpushed", "main", (2, 0)),
        ("w4", "waiting", "/code/waiting", "main", (0, 0)),
        ("w5", "working", "/code/working", "main", (0, 0)),
    ]);
    let repos = [repo(vec![shown("/code/dirty", &["w2"], 2, 0)])];
    let rows = [
        row("local", "w4", State::Blocked, "Should I continue?", 1),
        row("local", "w5", State::Working, "Running tests", 1),
    ];
    let cards = cards(
        [Host {
            index: 0,
            id: "local",
            label: None,
            snapshot: &local,
            repos: Some(&repos),
        }],
        &rows,
    );
    assert_eq!(
        names(&cards),
        ["quiet", "dirty", "unpushed", "waiting", "working"]
    );
    let waiting: Vec<bool> = cards.iter().map(Card::waiting).collect();
    assert_eq!(waiting, [false, false, false, true, false]);
}

#[test]
fn a_read_checkout_supplies_files_lines_branch_and_sync() {
    let local = snapshot(&[("w1", "app", "/code/app/crates/ui", "stale", (0, 0))]);
    let repos = [repo(vec![
        // A pane of the workspace in another checkout does not win over the
        // one holding its folder.
        shown("/code/other", &["w1"], 9, 0),
        shown("/code/app", &["w1"], 2, 3),
    ])];
    let card = &cards(
        [Host {
            index: 0,
            id: "local",
            label: None,
            snapshot: &local,
            repos: Some(&repos),
        }],
        &[],
    )[0];
    assert_eq!(
        card.dirty,
        Some(Dirty {
            files: 2,
            added: 6,
            removed: 2
        })
    );
    assert_eq!(card.branch.as_deref(), Some("read-branch"));
    assert_eq!((card.ahead, card.behind), (3, 0));
}

#[test]
fn a_remote_host_keeps_its_daemons_git_state_and_reports_no_files() {
    let remote = snapshot(&[("w1", "api", "/srv/api", "fix/backoff", (2, 1))]);
    let card = &cards(
        [Host {
            index: 1,
            id: "nas",
            label: Some("ivan@nas"),
            snapshot: &remote,
            repos: None,
        }],
        &[],
    )[0];
    assert_eq!(card.dirty, None, "no read of that host's files was made");
    assert_eq!(card.branch.as_deref(), Some("fix/backoff"));
    assert_eq!((card.ahead, card.behind), (2, 1));
    assert_eq!(card.host.as_deref(), Some("ivan@nas"));
    // A workspace outside Git on a read host is clean, not unknown.
    let local = snapshot(&[("w1", "notes", "/notes", "", (0, 0))]);
    let card = &cards(
        [Host {
            index: 0,
            id: "local",
            label: None,
            snapshot: &local,
            repos: Some(&[]),
        }],
        &[],
    )[0];
    assert_eq!(card.dirty, Some(Dirty::default()));
}

#[test]
fn the_most_pressing_agent_leads_and_the_rest_are_counted() {
    let local = snapshot(&[("w1", "app", "/code/app", "main", (0, 0))]);
    let rows = [
        row("local", "w1", State::Working, "Building", 5),
        row("local", "w1", State::Blocked, "Older question", 1),
        row("local", "w1", State::Blocked, "Newer question", 9),
        // Another host's workspace with the same id is not this one's.
        row("nas", "w1", State::Failed, "Elsewhere", 10),
    ];
    let card = &cards(
        [Host {
            index: 0,
            id: "local",
            label: None,
            snapshot: &local,
            repos: None,
        }],
        &rows,
    )[0];
    let agent = card.agent.as_ref().unwrap();
    assert_eq!(agent.state, State::Blocked);
    assert_eq!(agent.title, "Newer question");
    assert_eq!(agent.count, 3);
}

#[test]
fn the_summary_counts_cards_and_distinct_hosts() {
    let local = snapshot(&[
        ("w1", "a", "/a", "main", (1, 0)),
        ("w2", "b", "/b", "main", (0, 0)),
    ]);
    let remote = snapshot(&[("w1", "c", "/c", "main", (1, 0))]);
    let repos = [repo(vec![shown("/b", &["w2"], 1, 0)])];
    let rows = [row("local", "w1", State::Blocked, "?", 1)];
    let all = cards(
        [
            Host {
                index: 0,
                id: "local",
                label: None,
                snapshot: &local,
                repos: Some(&repos),
            },
            Host {
                index: 1,
                id: "nas",
                label: Some("nas"),
                snapshot: &remote,
                repos: None,
            },
        ],
        &rows,
    );
    assert_eq!(
        Summary::of(&all),
        Summary {
            waiting: 1,
            dirty: 1,
            unpushed: 2,
            hosts: 2
        }
    );
    assert_eq!(Summary::of(&[]), Summary::default());
}

#[test]
fn marks_take_initials_and_a_stable_hue() {
    assert_eq!(monogram("herdr-gpui"), "HG");
    assert_eq!(monogram("venturi_paragliding v2"), "VP");
    assert_eq!(monogram("dotfiles"), "DO");
    assert_eq!(monogram("ñandú"), "ÑA");
    assert_eq!(monogram("--"), "?");
    assert_eq!(hue("herdr-gpui"), hue("herdr-gpui"));
    for name in ["a", "herdr-gpui", "dotfiles", "", "ansible-recipes"] {
        assert!((1..=6).contains(&hue(name)), "{name}");
    }
}
