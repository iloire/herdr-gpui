use super::page::{bounds, click, fixture};
use crate::{folders::Part, palette::Filter};
use std::sync::Arc;

#[gpui::test]
fn the_new_button_opens_the_palette_on_its_projects(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    assert!(bounds(cx, "folders-new").is_some());
    click(cx, "folders-new");
    view.read_with(cx, |view, _| {
        let palette = view.menu.palette.as_ref().expect("palette open");
        assert_eq!(palette.filter(), Filter::Projects);
        assert!(view.folders.follow_opened);
    });
}

/// The workspace the window arrives in after the + is the project opened,
/// and Folders selects it on its Home, once.
#[gpui::test]
fn the_folder_arrived_in_after_the_new_button_is_selected_on_home(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = super::page::endpoint(&view, cx);
    click(cx, "folders-new");
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.dismiss_menu(window, cx);
            view.folders.part = Part::Changes;
            Arc::make_mut(view.live.snapshot.as_mut().unwrap()).focused_workspace_id =
                Some("w1".into());
            view.folder_arrived(window, cx);
        })
    });
    view.read_with(cx, |view, _| {
        assert_eq!(view.folders.selected, Some((host, "w1".into())));
        assert_eq!(view.folders.part, Part::Home);
        assert!(!view.folders.follow_opened);
    });
}
