use crate::code_edit::display::Shown;

#[test]
fn tabs_widen_to_the_next_stop() {
    let shown = Shown::of("a\tb\t\tc");
    assert_eq!(shown.text, "a   b       c");
    assert_eq!(shown.drawn(2), 4);
    assert_eq!(shown.drawn(4), 8);
}

#[test]
fn a_click_inside_a_tab_lands_before_it() {
    let shown = Shown::of("a\tb");
    assert_eq!(shown.offset(2), 1);
    assert_eq!(shown.offset(4), 2);
    assert_eq!(shown.offset(99), 3);
}

#[test]
fn hidden_characters_show_as_a_mark() {
    let shown = Shown::of("a\u{202e}b\u{7}");
    assert_eq!(shown.text, "a\u{fffd}b\u{fffd}");
    // The override is three bytes in the file and three drawn, as the mark.
    assert_eq!(shown.drawn(4), 4);
    assert_eq!(shown.drawn(5), 5);
}
