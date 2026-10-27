use super::*;

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// The panel keeps its width as the filters change, though only Go To
/// shows the preview that width makes room for.
#[gpui::test]
fn switching_filters_keeps_the_palette_width(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        crate::bind_keys(cx);
        fixture_window(window, cx)
    });
    cx.simulate_resize(size(px(1400.), px(900.)));
    cx.update(|window, cx| {
        view.update(cx, |view, cx| view.open_palette(Filter::All, window, cx));
    });
    draw(cx);
    let width = cx.debug_bounds("palette-status").unwrap().size.width;
    for filter in [
        "palette-filter-navigation",
        "palette-filter-commands",
        "palette-filter-projects",
        "palette-filter-notes",
        "palette-filter-all",
    ] {
        let tab = cx.debug_bounds(filter).unwrap().center();
        cx.simulate_click(tab, Modifiers::none());
        draw(cx);
        assert_eq!(
            cx.debug_bounds("palette-status").unwrap().size.width,
            width,
            "{filter}"
        );
    }
}

/// Opened to pick a project, the palette names that job and offers no
/// other filter, by click or key.
#[gpui::test]
fn the_project_picker_shows_projects_alone(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        crate::bind_keys(cx);
        fixture_window(window, cx)
    });
    cx.simulate_resize(size(px(1400.), px(900.)));
    cx.update(|window, cx| {
        view.update(cx, |view, cx| view.open_projects_palette(window, cx));
    });
    draw(cx);
    assert!(cx.debug_bounds("palette-filter-all").is_none());
    assert!(cx.debug_bounds("palette-agent-filter-all-agents").is_none());
    assert!(cx.debug_bounds("palette-preview").is_none());
    assert!(cx.debug_bounds("palette-no-project-roots").is_some());
    let narrow = cx.debug_bounds("palette-status").unwrap().size.width;

    cx.simulate_keystrokes("ctrl-tab alt-b");
    view.read_with(cx, |view, cx| {
        let palette = view.menu.palette.as_ref().unwrap();
        assert_eq!(palette.filter(), Filter::Projects);
        assert_eq!(palette.scope(), Scope::OpenProject);
        assert_eq!(
            palette.search.read(cx).placeholder(),
            "Search recent folders, or choose one..."
        );
    });

    cx.simulate_keystrokes("escape");
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open_palette(Filter::Projects, window, cx)
        });
    });
    draw(cx);
    assert!(cx.debug_bounds("palette-status").unwrap().size.width > narrow);
}
