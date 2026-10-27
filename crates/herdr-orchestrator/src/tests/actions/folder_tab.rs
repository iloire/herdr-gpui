use super::*;
use remove::run_pointing_at;

/// The run's recorded workspace set to `id`, as runs dispatched before the
/// agent moved into a folder tab recorded the worktree's own.
fn recorded_in(site: &Site, id: &str) -> String {
    let mut store = Store::open(&site.database).unwrap();
    let mut run = store.runs().unwrap().remove(0);
    let session = store.sessions().unwrap().remove(0);
    run.workspace.as_mut().unwrap().id = id.into();
    let run_id = run.id.clone();
    store.update_run(&run, Some(&session)).unwrap();
    run_id
}

#[test]
fn a_failed_move_removes_the_worktree_through_its_own_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let site = site(dir.path(), "pane move");
    let mut own =
        serde_json::from_str::<serde_json::Value>(&snapshot(&[("w1", false), ("w9", true)]))
            .unwrap();
    own["result"]["snapshot"]["workspaces"][1]["worktree"]["checkout_path"] =
        "/tmp/wt/377-icons".into();
    std::fs::write(dir.path().join("home/snapshot"), own.to_string()).unwrap();

    let error = perform(
        &site,
        &Action::Dispatch(Box::new(request())),
        &AtomicBool::new(false),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        Error::Script {
            operation: "Opening the agent's tab",
            ..
        }
    ));
    let calls = calls(dir.path());
    assert!(
        !calls.iter().any(|call| call.starts_with("agent ")),
        "{calls:?}"
    );
    assert_eq!(
        calls.last().unwrap(),
        "worktree remove --workspace w9 --force"
    );
}

#[test]
fn a_run_in_its_own_worktree_workspace_is_removed_through_herdr() {
    let dir = tempfile::tempdir().unwrap();
    let site = site(dir.path(), "");
    let cancelled = AtomicBool::new(false);
    perform(&site, &Action::Dispatch(Box::new(request())), &cancelled).unwrap();
    run_pointing_at(&site, Some("/r/w9".into()));
    let id = recorded_in(&site, "w9");

    perform(&site, &Action::Remove { run: id }, &cancelled).unwrap();

    assert_eq!(
        calls(dir.path()).last().unwrap(),
        "worktree remove --workspace w9 --force"
    );
}

#[test]
fn a_reused_workspace_id_is_never_removed_through_herdr() {
    let dir = tempfile::tempdir().unwrap();
    let site = site(dir.path(), "");
    let cancelled = AtomicBool::new(false);
    perform(&site, &Action::Dispatch(Box::new(request())), &cancelled).unwrap();
    // `w9` is now another checkout than the run's `/tmp/wt/377-icons`.
    let id = recorded_in(&site, "w9");

    perform(&site, &Action::Remove { run: id }, &cancelled).unwrap();

    let calls = calls(dir.path());
    assert!(
        !calls.iter().any(|call| call.starts_with("worktree remove")),
        "{calls:?}"
    );
    assert_eq!(calls.last().unwrap(), "pane close w1:p9");
}

#[test]
fn a_pane_outside_the_checkout_is_left_open() {
    let dir = tempfile::tempdir().unwrap();
    let site = site(dir.path(), "");
    let cancelled = AtomicBool::new(false);
    perform(&site, &Action::Dispatch(Box::new(request())), &cancelled).unwrap();
    let id = Store::open(&site.database).unwrap().runs().unwrap()[0]
        .id
        .clone();
    // Herdr gave the id to another pane, or its shell left the checkout;
    // a sibling directory sharing the checkout's name prefix is outside it.
    std::fs::write(
        dir.path().join("home/snapshot"),
        snapshot_with(&[("w1", false)], &[agent_pane("/tmp/wt/377-icons-old")]),
    )
    .unwrap();

    perform(&site, &Action::Remove { run: id }, &cancelled).unwrap();

    assert_eq!(calls(dir.path()).last().unwrap(), "api snapshot");
}
