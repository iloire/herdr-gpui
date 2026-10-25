use super::{ClientShellWorkspace, FolderCard, Theme};

const ROOT: &str = if cfg!(windows) {
    "C:/fixture/agent-launcher"
} else {
    "/fixture/agent-launcher"
};

fn workspace(cwd: &str, linked: bool) -> ClientShellWorkspace {
    serde_json::from_value(serde_json::json!({
        "workspace_id": "w0", "active_tab_id": "t0", "new_workspace_cwd": cwd,
        "number": 1, "label": "agent-launcher", "custom_label": false,
        "branch": "develop",
        "worktree": { "key": format!("{ROOT}/.git"), "label": "agent-launcher", "is_linked_worktree": linked },
        "tokens": [], "focused": false, "agent_status": "idle"
    }))
    .unwrap()
}

fn card(workspace: &ClientShellWorkspace, local: bool) -> Option<FolderCard> {
    let config = crate::config::Config::default();
    FolderCard::of(
        workspace,
        "agent-launcher",
        local,
        &Theme::default(),
        &config.terminal,
    )
}

#[test]
fn a_main_checkout_inside_its_repository_has_not_left_it() {
    let card = card(&workspace(&format!("{ROOT}/crates"), false), true).unwrap();
    assert!(!card.left_checkout);
    assert_eq!(card.branch.as_deref(), Some("develop"));
}

#[test]
fn a_main_checkout_whose_shell_moved_away_is_flagged() {
    let moved = format!("{ROOT}-elsewhere");
    assert!(card(&workspace(&moved, false), true).unwrap().left_checkout);
    // A linked checkout's own path is not in the snapshot, so it is never judged.
    assert!(!card(&workspace(&moved, true), true).unwrap().left_checkout);
}

#[test]
fn no_card_without_a_folder() {
    assert!(card(&workspace("", false), true).is_none());
}

#[test]
fn only_this_computers_folders_are_written_from_home() {
    let Some(home) = herdr_client::local_path::home_dir() else {
        return;
    };
    let folder = home.join("code").join("herdr").display().to_string();
    let local = card(&workspace(&folder, false), true).unwrap();
    assert!(
        local.shown_folder().starts_with("~"),
        "{}",
        local.shown_folder()
    );
    let remote = card(&workspace(&folder, false), false).unwrap();
    assert_eq!(remote.shown_folder().as_ref(), folder);
}
