use super::*;
use crate::files::Listing;
use herdr_vcs::code_index::Index;

#[gpui::test]
fn a_file_opened_from_the_tree_shows_at_the_first_click(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();
    let (view, cx) = window(cx);
    files(&view, cx);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let index = Index::of(dir.path(), &["main.rs"], Vec::new());
            view.files.listing = Some(Listing::of(index));
            view.open_files_entry("main.rs", false, window, cx);
        })
    });
    let id = view.read_with(cx, |view, _| *view.code_views.keys().next().unwrap());
    assert_eq!(listed(&view, id, cx), (true, true));
    // The next tick settles the groups before it counts the file tabs: it
    // must not take the new tab for a closed one.
    cx.update(|window, cx| view.update(cx, |view, cx| view.settle_groups(window, cx)));
    assert_eq!(listed(&view, id, cx), (true, true));
}
