use super::*;

/// Code files set aside, as leaving the Files view does.
fn files(pick: &Pick) -> bool {
    matches!(pick, Pick::Page(id) if *id == TabId::test(1) || *id == TabId::test(2))
}

#[test]
fn a_lone_group_on_a_file_follows_the_terminal_and_gets_it_back() {
    let (mut layout, _, first) = layout();
    layout.choose(first, page(1));
    let taken = layout.set_aside(files);
    assert_eq!(taken, [(first, page(1))]);
    assert_eq!(shown(&layout, "t1"), [Terminal]);
    layout.bring_back(taken);
    assert_eq!(shown(&layout, "t1"), [Page(TabId::test(1))]);
}

#[test]
fn a_split_group_on_a_file_turns_to_its_last_other_tab() {
    let (mut layout, mut ids, first) = layout();
    let second = ids.next();
    layout.split(first, second, Some("t1"), Vec::new());
    layout.choose(second, page(3));
    layout.choose(second, page(1));
    layout.choose(second, page(2));
    let taken = layout.set_aside(files);
    assert_eq!(taken, [(second, page(2))]);
    assert_eq!(
        shown(&layout, "t1"),
        [Terminal, Page(TabId::test(3))],
        "the terminal group keeps its terminal"
    );
    layout.bring_back(taken);
    assert_eq!(shown(&layout, "t1")[1], Page(TabId::test(2)));
}

#[test]
fn a_group_with_only_files_shows_nothing_beside_another() {
    let (mut layout, mut ids, first) = layout();
    let second = ids.next();
    layout.split(first, second, Some("t1"), Vec::new());
    layout.choose(second, page(1));
    layout.set_aside(files);
    assert_eq!(shown(&layout, "t1"), [Terminal, Empty]);
}
