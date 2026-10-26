use super::*;
use crate::{
    files::{Listing, Opening},
    menu::Page,
    sidebar::layout_tests::full_draw,
};
use gpui::{Modifiers, MouseButton};
use herdr_vcs::code_index::Index;

/// Shows the Files tree of `files` under `dir`, its folders closed.
fn tree(
    view: &Entity<HerdrWindow>,
    dir: &std::path::Path,
    files: &[&str],
    cx: &mut VisualTestContext,
) {
    super::files(view, cx);
    cx.update(|window, cx| {
        view.update(cx, |view, _| {
            view.files.listing = Some(Listing::of(Index::of(dir, files, Vec::new())));
            view.files.status = crate::files::Status::Ready;
            view.files.collapse_all();
        });
        window.refresh();
        full_draw(window, cx).clear(cx);
    });
}

#[gpui::test]
fn a_file_opened_from_the_tree_shows_at_the_first_click(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();
    let (view, cx) = window(cx);
    tree(&view, dir.path(), &["main.rs"], cx);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open_files_entry("main.rs", Opening::Tab, window, cx)
        })
    });
    let id = view.read_with(cx, |view, _| *view.code_views.keys().next().unwrap());
    assert_eq!(listed(&view, id, cx), (true, true));
    // The next tick settles the groups before it counts the file tabs: it
    // must not take the new tab for a closed one.
    cx.update(|window, cx| view.update(cx, |view, cx| view.settle_groups(window, cx)));
    assert_eq!(listed(&view, id, cx), (true, true));
}

#[gpui::test]
fn right_clicking_a_markdown_file_opens_its_source(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("README.md"), "# Read me\n").unwrap();
    let (view, cx) = window(cx);
    tree(&view, dir.path(), &["README.md"], cx);
    let row = cx.debug_bounds("files-file").unwrap();
    cx.simulate_mouse_down(row.center(), MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(row.center(), MouseButton::Right, Modifiers::default());
    cx.update(|window, cx| {
        window.refresh();
        full_draw(window, cx).clear(cx);
    });
    assert_eq!(
        cx.update(|_, cx| view.read(cx).menu.page),
        Some(Page::Files)
    );
    // Open Rendered, then Open as Source.
    let source = cx.debug_bounds("files-menu-1").unwrap();
    cx.simulate_click(source.center(), Modifiers::default());
    assert_eq!(cx.update(|_, cx| view.read(cx).menu.page), None);
    let opened = view.read_with(cx, |view, cx| {
        let id = *view.code_views.keys().next()?;
        cx.try_global::<Store>()?.get(id)?.location.clone()
    });
    assert!(
        matches!(&opened, Some(Location::Code { file }) if file.path.ends_with("README.md")),
        "{opened:?}"
    );
}
