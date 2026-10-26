use super::{draw, window};
use crate::{HerdrWindow, browser::TabId, code_view::is_picture, editor::EditorTarget};
use gpui::{Entity, TestAppContext, VisualTestContext};
use std::path::Path;

fn write_png(path: &Path, width: u32, height: u32) {
    image::RgbaImage::from_pixel(width, height, image::Rgba([10, 20, 30, 255]))
        .save(path)
        .unwrap();
}

fn open<'a>(
    path: &Path,
    cx: &'a mut TestAppContext,
) -> (Entity<HerdrWindow>, TabId, &'a mut VisualTestContext) {
    let (view, cx) = window(cx);
    let target = EditorTarget {
        path: path.to_owned(),
        line: None,
    };
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_code_view(&target, window, cx)));
    cx.run_until_parked();
    draw(cx);
    let id = view.read_with(cx, |view, _| *view.code_views.keys().next().unwrap());
    (view, id, cx)
}

fn size(view: &Entity<HerdrWindow>, id: TabId, cx: &mut VisualTestContext) -> Option<(u32, u32)> {
    view.read_with(cx, |view, _| match &view.code_views[&id].picture {
        Some(Ok(picture)) => Some(picture.size),
        _ => None,
    })
}

#[test]
fn pictures_are_known_by_their_extension() {
    for path in ["a.png", "b.JPG", "c.jpeg", "d.gif", "e.WebP"] {
        assert!(is_picture(Path::new(path)), "{path}");
    }
    for path in ["a.svg", "b.rs", "png", ".png.txt", "c"] {
        assert!(!is_picture(Path::new(path)), "{path}");
    }
}

#[gpui::test]
fn a_picture_tab_draws_the_picture_and_never_edits_it(cx: &mut TestAppContext) {
    let dir = tempfile::Builder::new().prefix("pic").tempdir().unwrap();
    let path = dir.path().join("shot.png");
    write_png(&path, 20, 10);
    let before = std::fs::read(&path).unwrap();
    let (view, id, cx) = open(&path, cx);
    assert_eq!(size(&view, id, cx), Some((20, 10)));
    assert!(cx.debug_bounds("code-picture").is_some());
    assert!(cx.debug_bounds("code-picture-size").is_some());
    for text_only in ["code-edit", "code-outline-toggle", "code-open-editor"] {
        assert!(cx.debug_bounds(text_only).is_none(), "{text_only}");
    }

    // Typing reaches no editor, and closing the tab writes nothing.
    cx.simulate_input("x");
    assert!(!view.read_with(cx, |view, cx| view.unsaved_code_edits(id, cx)));

    // An agent rewrites it: the tab reads it again, and the old texture
    // is freed on the next tick, as is the last once the tab closes.
    write_png(&path, 30, 5);
    cx.update(|_, cx| view.update(cx, |view, cx| view.load_code_view(id, cx)));
    cx.run_until_parked();
    assert_eq!(size(&view, id, cx), Some((30, 5)));
    assert_eq!(
        view.read_with(cx, |view, _| view.code_views[&id].released.len()),
        1
    );
    cx.update(|window, cx| view.update(cx, |view, cx| view.release_code_pictures(window, cx)));
    assert!(view.read_with(cx, |view, _| view.code_views[&id].released.is_empty()));
    let after = std::fs::read(&path).unwrap();
    assert_ne!(before, after);

    cx.update(|window, cx| view.update(cx, |view, cx| view.close_browser_tab(id, window, cx)));
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.release_code_pictures(window, cx);
            assert!(view.code_views[&id].picture.is_none());
            view.poll_code_views(cx);
        })
    });
    cx.run_until_parked();
    assert!(view.read_with(cx, |view, _| view.code_views.is_empty()));
    assert_eq!(std::fs::read(&path).unwrap(), after);
}

#[gpui::test]
fn a_picture_that_does_not_decode_says_so(cx: &mut TestAppContext) {
    let dir = tempfile::Builder::new().prefix("pic").tempdir().unwrap();
    let path = dir.path().join("broken.png");
    std::fs::write(&path, b"\x89PNG not really").unwrap();
    let (view, id, cx) = open(&path, cx);
    assert!(view.read_with(cx, |view, _| matches!(
        view.code_views[&id].picture,
        Some(Err(crate::backdrop::Error::Decode { .. }))
    )));
    assert!(cx.debug_bounds("code-picture-failed").is_some());
    assert!(cx.debug_bounds("code-picture").is_none());
}
