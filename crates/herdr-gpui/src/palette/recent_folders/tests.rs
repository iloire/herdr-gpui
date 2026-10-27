use super::*;

/// A snapshot with one workspace per root, each launched in it.
fn open(roots: &[&str]) -> Arc<ClientShellSnapshot> {
    let mut snapshot: ClientShellSnapshot = serde_json::from_str(include_str!(
        "../../../../herdr-protocol/tests/fixtures/endpoint-snapshot-v1.json"
    ))
    .unwrap();
    let (workspace, tab, pane) = (
        snapshot.workspaces[0].clone(),
        snapshot.tabs[0].clone(),
        snapshot.panes[0].clone(),
    );
    snapshot.workspaces.clear();
    snapshot.tabs.clear();
    snapshot.panes.clear();
    for (index, root) in roots.iter().enumerate() {
        let id = format!("w{index}");
        let mut workspace = workspace.clone();
        workspace.workspace_id = id.clone();
        let mut tab = tab.clone();
        tab.workspace_id = id.clone();
        tab.tab_id = format!("{id}:t1");
        let mut pane = pane.clone();
        pane.workspace_id = id.clone();
        pane.tab_id = tab.tab_id.clone();
        pane.pane_id = format!("{id}:p1");
        pane.cwd = Some((*root).into());
        snapshot.workspaces.push(workspace);
        snapshot.tabs.push(tab);
        snapshot.panes.push(pane);
    }
    Arc::new(snapshot)
}

fn paths(recent: &RecentFolders) -> Vec<&str> {
    recent
        .folders
        .iter()
        .map(|folder| folder.path.to_str().unwrap())
        .collect()
}

#[test]
fn folders_that_open_or_close_move_to_the_front() {
    let mut recent = RecentFolders::default();
    recent.observe(&open(&["/a", "/b"]), 1);
    assert_eq!(paths(&recent), ["/a", "/b"]);
    recent.observe(&open(&["/a", "/b", "/c"]), 2);
    assert_eq!(paths(&recent), ["/c", "/a", "/b"]);
    // Closing /a makes it the last used.
    recent.observe(&open(&["/b", "/c"]), 3);
    assert_eq!(paths(&recent), ["/a", "/c", "/b"]);
    assert_eq!(recent.folders[0].at, 3);
}

#[test]
fn an_unchanged_snapshot_changes_nothing() {
    let mut recent = RecentFolders::default();
    let snapshot = open(&["/a"]);
    recent.observe(&snapshot, 1);
    recent.observe(&snapshot, 2);
    recent.observe(&open(&["/a"]), 3);
    assert_eq!(
        recent.folders,
        [Folder {
            path: "/a".into(),
            at: 1
        }]
    );
}

#[test]
fn relative_and_oversized_roots_are_not_remembered_and_the_list_is_bounded() {
    let mut recent = RecentFolders::default();
    let long = format!("/{}", "a".repeat(MAX_PATH_BYTES));
    recent.observe(&open(&["relative", &long, "/ok"]), 1);
    assert_eq!(paths(&recent), ["/ok"]);
    let many: Vec<String> = (0..=MAX_FOLDERS)
        .map(|index| format!("/f{index}"))
        .collect();
    recent.observe(
        &open(&many.iter().map(String::as_str).collect::<Vec<_>>()),
        2,
    );
    assert_eq!(recent.folders.len(), MAX_FOLDERS);
}

#[test]
fn saved_folders_are_read_back_and_a_damaged_file_starts_empty() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let mut recent = RecentFolders::at(Some(file.clone()));
    recent.observe(&open(&["/a", "/b"]), 1);
    recent.writer.take().unwrap().finish();
    assert_eq!(paths(&RecentFolders::at(Some(file.clone()))), ["/a", "/b"]);
    std::fs::write(&file, r#"{"folders":[{"path":"relative","at":1}]}"#).unwrap();
    assert!(RecentFolders::at(Some(file)).folders.is_empty());
}

#[test]
fn closed_lists_existing_folders_that_are_not_open_once_each() {
    let dir = tempfile::tempdir().unwrap();
    let root = canonical::canonicalize(dir.path()).unwrap();
    for name in ["kept", "open"] {
        std::fs::create_dir(root.join(name)).unwrap();
    }
    let recent = vec![
        root.join("kept"),
        root.join("missing"),
        root.join("open"),
        root.join("kept"),
    ];
    let open = [root.join("open").to_string_lossy().into_owned()];
    let projects = closed(recent, &open);
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].label, "kept");
    assert_eq!(projects[0].path, root.join("kept"));
}

#[test]
fn nearby_lists_the_siblings_of_used_folders_but_never_homes_children() {
    let dir = tempfile::tempdir().unwrap();
    let code = canonical::canonicalize(dir.path()).unwrap().join("code");
    for name in ["used", "sibling", ".hidden"] {
        std::fs::create_dir_all(code.join(name)).unwrap();
    }
    let open = [code.join("used").to_string_lossy().into_owned()];
    let found = nearby(&[code.join("used")], &open, &Default::default());
    let mut labels: Vec<_> = found.projects.iter().map(|p| p.label.as_str()).collect();
    labels.sort_unstable();
    assert_eq!(labels, ["sibling", "used"]);
    assert!(found.errors.is_empty());

    let home = crate::config::home().unwrap();
    let at_home = nearby(
        &[home.join("notes"), PathBuf::from("/")],
        &[],
        &Default::default(),
    );
    assert!(at_home.projects.is_empty());
}
