use super::*;

/// Opened from the Folders list's +, the picker leads with the folder
/// dialog, then the folders opened before, even with no roots configured.
#[gpui::test]
fn the_folder_picker_offers_a_dialog_and_recent_folders(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old-project");
    std::fs::create_dir(&old).unwrap();
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|window, cx| {
        let fixture = view.read(cx).live.snapshot.clone().unwrap();
        let mut there = (*fixture).clone();
        there.panes[0].cwd = Some(old.to_string_lossy().into_owned());
        let mut recent = RecentFolders::default();
        recent.observe(&Arc::new(there), 1);
        // The workspace closes: the fixture's own folder is open instead.
        recent.observe(&fixture, 2);
        cx.set_global(recent);
        view.update(cx, |view, cx| view.open_projects_palette(window, cx));
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        let palette = view.menu.palette.as_ref().unwrap();
        let rows: Vec<_> = palette
            .filtered
            .iter()
            .map(|hit| {
                let entry = &palette.entries[hit.index];
                (entry.label.to_string(), entry.badge.to_string())
            })
            .collect();
        assert_eq!(rows[0], ("Choose a folder…".into(), "Browse".into()));
        assert!(
            rows.contains(&("old-project".into(), "Recent".into())),
            "{rows:?}"
        );
    });
    cx.update(|_, cx| {
        view.update(cx, |view, cx| view.filter_palette("old", cx));
    });
    view.read_with(cx, |view, _| {
        let palette = view.menu.palette.as_ref().unwrap();
        let first = &palette.entries[palette.filtered[0].index];
        assert_eq!(first.label.as_ref(), "old-project");
    });
}
