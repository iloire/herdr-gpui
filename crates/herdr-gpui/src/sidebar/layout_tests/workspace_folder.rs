use super::*;

fn connected(cx: &mut gpui::TestAppContext) -> (Entity<HerdrWindow>, &mut gpui::VisualTestContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        crate::bind_keys(cx);
        let mut view = fixture_window(window, cx);
        view.live.status = crate::state::ConnectionStatus::Connected;
        view.active = true;
        view
    });
    cx.simulate_resize(size(px(900.), px(700.)));
    cx.update(|window, cx| full_draw(window, cx).clear(cx));
    (view, cx)
}

/// Resting the pointer on a workspace row shows the folder its new tabs
/// start in.
#[gpui::test]
fn hovering_a_workspace_row_shows_its_folder_card(cx: &mut gpui::TestAppContext) {
    let (_view, cx) = connected(cx);
    let row = cx.debug_bounds("row-herdr").unwrap();
    cx.simulate_mouse_move(row.center(), None, Modifiers::default());
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    cx.update(|window, cx| full_draw(window, cx).clear(cx));
    let card = cx.debug_bounds("folder-card").unwrap();
    let path = cx.debug_bounds("folder-card-path").unwrap();
    assert!(path.left() >= card.left() && path.right() <= card.right());
    assert!(cx.debug_bounds("folder-card-branch").is_some());
    assert!(cx.debug_bounds("folder-card-left-checkout").is_none());
}

/// The hover menu shows the folder itself, and a carried row shows nothing.
#[gpui::test]
fn no_folder_card_under_the_hover_menu_or_while_carried(cx: &mut gpui::TestAppContext) {
    let (view, cx) = connected(cx);
    view.update(cx, |view, _| {
        let workspace = view.live.snapshot.as_ref().unwrap().workspaces[0].clone();
        assert!(view.folder_card(&workspace, "herdr", true, false).is_some());
        assert!(view.folder_card(&workspace, "herdr", true, true).is_none());
        view.config.features.sidebar_hover_menu = true;
        assert!(view.folder_card(&workspace, "herdr", true, false).is_none());
    });
}

/// The menu header names the folder under the branch, and its copy button
/// puts the daemon's path, not the `~` form, on the clipboard.
#[gpui::test]
fn the_workspace_menu_shows_and_copies_the_folder(cx: &mut gpui::TestAppContext) {
    let (view, cx) = connected(cx);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open_workspace_menu("w0", point(px(20.), px(100.)), window, cx)
        });
        full_draw(window, cx).clear(cx);
    });
    let panel = cx.debug_bounds("menu-panel").unwrap();
    let branch = cx.debug_bounds("workspace-menu-branch").unwrap();
    let path = cx.debug_bounds("workspace-menu-folder-path").unwrap();
    let copy = cx.debug_bounds("workspace-menu-folder-copy").unwrap();
    assert!(path.top() >= branch.bottom(), "{path:?} {branch:?}");
    assert!(path.left() >= panel.left() && copy.right() <= panel.right());
    cx.simulate_click(copy.center(), Modifiers::default());
    cx.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("/tmp")
        );
        assert!(view.read(cx).menu.page.is_none());
    });
}

/// A workspace the daemon gives no folder has no folder line.
#[gpui::test]
fn no_folder_line_without_a_folder(cx: &mut gpui::TestAppContext) {
    let (view, cx) = connected(cx);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            Arc::make_mut(view.live.snapshot.as_mut().unwrap()).workspaces[0]
                .new_workspace_cwd
                .clear();
            view.open_workspace_menu("w0", point(px(20.), px(100.)), window, cx)
        });
        full_draw(window, cx).clear(cx);
    });
    assert!(cx.debug_bounds("workspace-menu-branch").is_some());
    assert!(cx.debug_bounds("workspace-menu-folder").is_none());
}
