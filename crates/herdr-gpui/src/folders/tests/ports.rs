use super::page::{bounds, draw, endpoint, fixture};

/// A folder's listening ports show on its own row, under its branch, so
/// they are seen without opening its Home; a folder with none shows no line.
#[gpui::test]
fn ports_ride_on_the_folder_row(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    let before = bounds(cx, &format!("folder-{host}-w1")).expect("row");
    assert!(bounds(cx, "folder-ports-w1").is_none());
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.listening_ports.seed(
                crate::usage::Host::Local,
                crate::listening_ports::parse("L 1 *:3000 node\nE 1 w1\n").unwrap(),
            );
            cx.notify();
        })
    });
    draw(cx);
    let row = bounds(cx, &format!("folder-{host}-w1")).expect("row");
    let line = bounds(cx, "folder-ports-w1").expect("ports line");
    assert!(row.contains(&line.center()), "{line:?} is not on {row:?}");
    assert!(
        row.size.height > before.size.height,
        "the line adds to the row"
    );
    assert!(bounds(cx, "folder-ports-w0").is_none());
    assert!(bounds(cx, &format!("port-{host}-w1-3000")).is_some());
}
