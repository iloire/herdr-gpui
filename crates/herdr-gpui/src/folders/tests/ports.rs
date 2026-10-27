use super::page::{bounds, draw, endpoint, fixture};

/// A folder's listening ports show as the last of its parts, so only the
/// selected folder lists them and no folder row grows to hold them.
#[gpui::test]
fn ports_ride_under_the_selected_folder(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    let before = bounds(cx, &format!("folder-{host}-w0")).expect("row");
    assert!(bounds(cx, "folder-ports-w0").is_none());
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.listening_ports.seed(
                crate::usage::Host::Local,
                crate::listening_ports::parse("L 1 *:3000 node\nL 2 *:8093 vite\nE 1 w0\nE 2 w1\n")
                    .unwrap(),
            );
            cx.notify();
        })
    });
    draw(cx);
    let row = bounds(cx, &format!("folder-{host}-w0")).expect("row");
    let files = bounds(cx, "folder-part-files").expect("files part");
    let line = bounds(cx, "folder-ports-w0").expect("ports line");
    assert_eq!(
        row.size.height, before.size.height,
        "the row keeps its height"
    );
    assert!(
        line.top() >= files.bottom(),
        "{line:?} is not under {files:?}"
    );
    assert!(bounds(cx, &format!("port-{host}-w0-3000")).is_some());
    // The unselected folder hides its ports with its parts.
    assert!(bounds(cx, "folder-ports-w1").is_none());
    assert!(bounds(cx, &format!("port-{host}-w1-8093")).is_none());
}
