use super::*;

/// `worktree list` output naming `branch`'s linked checkout at `path`.
fn listing(branch: &str, path: &str) -> String {
    serde_json::json!({"result": {"worktrees": [
        {"path": "/r/w1", "branch": "main", "is_linked_worktree": false, "is_prunable": false},
        {"path": path, "branch": branch, "is_linked_worktree": true, "is_prunable": false},
    ]}})
    .to_string()
}

#[test]
fn a_branch_already_checked_out_gets_a_folder_tab_in_that_checkout() {
    let dir = tempfile::tempdir().unwrap();
    let site = site(dir.path(), "worktree create");
    std::fs::write(
        dir.path().join("home/worktrees"),
        listing("377-icons", "/tmp/wt/earlier"),
    )
    .unwrap();

    let done = perform(
        &site,
        &Action::Dispatch(Box::new(request())),
        &AtomicBool::new(false),
    )
    .unwrap();

    assert_eq!(done, "Agent started");
    let calls = calls(dir.path());
    assert!(calls[1].starts_with("worktree create --workspace w1 --branch 377-icons "));
    assert_eq!(calls[2], "worktree list --workspace w1");
    assert_eq!(
        calls[3],
        "tab create --workspace w1 --cwd /tmp/wt/earlier --no-focus"
    );
    assert!(calls[4].starts_with("agent start herdr-gpui-"));
    assert!(calls[4].ends_with(" --pane w1:p10 --timeout 90000"));
    assert!(calls[5].starts_with("agent prompt w1:p10 "));

    let store = Store::open(&site.database).unwrap();
    let run = store.runs().unwrap().remove(0);
    assert_eq!(run.state, RunState::Running);
    let workspace = run.workspace.unwrap();
    assert_eq!(workspace.id, "w1");
    assert_eq!(
        workspace.path.as_deref(),
        Some(Path::new("/tmp/wt/earlier"))
    );
    assert_eq!(store.sessions().unwrap()[0].pane_id, "w1:p10");
}

#[test]
fn a_failed_create_without_a_checkout_of_the_branch_reports_the_create() {
    let dir = tempfile::tempdir().unwrap();
    let site = site(dir.path(), "worktree create");
    std::fs::write(
        dir.path().join("home/worktrees"),
        listing("someone-else", "/tmp/wt/other"),
    )
    .unwrap();

    let error = perform(
        &site,
        &Action::Dispatch(Box::new(request())),
        &AtomicBool::new(false),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        Error::Script {
            operation: "Creating the worktree",
            ..
        }
    ));
    assert!(
        !calls(dir.path())
            .iter()
            .any(|call| call.starts_with("tab create"))
    );
    let run = Store::open(&site.database)
        .unwrap()
        .runs()
        .unwrap()
        .remove(0);
    assert_eq!(run.state, RunState::Failed);
}
