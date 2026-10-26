use super::page::{bounds, endpoint, fixture};
use super::{cards, snapshot};
use crate::folders::model::Host;

fn card_titles(label: &str, folder: &str) -> (String, Option<String>) {
    let shown = snapshot(&[("w1", label, folder, "main", (0, 0))]);
    let host = Host {
        index: 0,
        id: "local",
        label: None,
        snapshot: &shown,
        repos: None,
    };
    let card = cards([host], &[]).remove(0);
    (card.title().to_owned(), card.alias().map(str::to_owned))
}

/// A card is titled by its folder, so a workspace renamed "loops" in
/// `~/notes` still reads as notes, with its label beside it.
#[test]
fn a_card_is_titled_by_its_folder_and_keeps_a_different_label() {
    assert_eq!(
        card_titles("loops", "/Users/me/notes"),
        ("notes".into(), Some("loops".into()))
    );
    assert_eq!(
        card_titles("notes", "/Users/me/notes/"),
        ("notes".into(), None)
    );
}

/// Without a folder from the daemon, the label is all there is to show.
#[test]
fn a_card_without_a_folder_keeps_its_label() {
    assert_eq!(card_titles("scratch", ""), ("scratch".into(), None));
    assert_eq!(card_titles("root", "/"), ("root".into(), None));
}

/// Every folder row shows where the folder is.
#[gpui::test]
fn each_row_shows_its_path(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    for workspace in ["w0", "w1"] {
        let row = bounds(cx, &format!("folder-{host}-{workspace}")).expect("row");
        let path = bounds(cx, &format!("folder-path-{workspace}")).expect("path line");
        assert!(row.contains(&path.center()), "{path:?} is not on {row:?}");
    }
}
