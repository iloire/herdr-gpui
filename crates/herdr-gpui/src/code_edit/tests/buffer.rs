use crate::code_edit::buffer::Buffer;

fn at(text: &str, cursor: usize) -> Buffer {
    let mut buffer = Buffer::new(text.to_owned());
    buffer.select_to(cursor, false);
    buffer
}

#[test]
fn lines_index_their_starts_and_exclude_the_newline() {
    let buffer = Buffer::new("one\ntwo\n\nfour".to_owned());
    assert_eq!(buffer.line_count(), 4);
    assert_eq!(buffer.line(0), "one");
    assert_eq!(buffer.line(2), "");
    assert_eq!(buffer.line(3), "four");
    assert_eq!(buffer.line_of(4), 1);
    assert_eq!(buffer.line_of(3), 0);
    assert_eq!(buffer.line_of(14), 3);
}

#[test]
fn typing_joins_one_undo_step_until_a_move() {
    let mut buffer = at("", 0);
    for c in ["a", "b", "c"] {
        buffer.insert(c);
    }
    buffer.select_to(1, false);
    buffer.insert("X");
    assert_eq!(buffer.text(), "aXbc");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "abc");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "");
    assert!(!buffer.undo());
    assert!(buffer.redo());
    assert_eq!(buffer.text(), "abc");
    assert_eq!(buffer.cursor, 3);
}

#[test]
fn a_space_ends_a_typed_word_for_undo() {
    let mut buffer = at("", 0);
    for c in ["a", "b", " ", "c"] {
        buffer.insert(c);
    }
    buffer.undo();
    assert_eq!(buffer.text(), "ab ");
    buffer.undo();
    assert_eq!(buffer.text(), "ab");
}

#[test]
fn a_new_edit_drops_what_could_be_redone() {
    let mut buffer = at("abc", 3);
    buffer.delete_to(2);
    buffer.undo();
    assert_eq!(buffer.selection(), 3..3);
    buffer.insert("d");
    assert!(!buffer.redo());
    assert_eq!(buffer.text(), "abcd");
}

#[test]
fn undo_restores_the_selection_it_replaced() {
    let mut buffer = Buffer::new("hello world".to_owned());
    buffer.select(6..11);
    buffer.insert("there");
    assert_eq!(buffer.text(), "hello there");
    buffer.undo();
    assert_eq!(buffer.text(), "hello world");
    assert_eq!(buffer.selection(), 6..11);
}

#[test]
fn enter_keeps_the_line_indent() {
    let mut buffer = at("fn x() {\n\t    body", 18);
    buffer.newline();
    assert_eq!(buffer.text(), "fn x() {\n\t    body\n\t    ");
    assert_eq!(buffer.cursor, buffer.text().len());
}

#[test]
fn pasted_carriage_returns_become_newlines() {
    let mut buffer = at("", 0);
    buffer.insert("a\r\nb\rc");
    assert_eq!(buffer.text(), "a\nb\nc");
    assert_eq!(buffer.line_count(), 3);
}

#[test]
fn up_and_down_keep_the_column_across_short_lines() {
    let mut buffer = at("abcdef\nab\nabcdef", 5);
    buffer.move_vertical(1, false);
    assert_eq!(buffer.cursor, 9);
    buffer.move_vertical(1, false);
    assert_eq!(buffer.cursor, 15);
    buffer.move_vertical(-5, false);
    assert_eq!(buffer.cursor, 5);
    buffer.move_vertical(-1, false);
    assert_eq!(buffer.cursor, 0);
    buffer.move_vertical(9, true);
    assert_eq!(buffer.cursor, buffer.text().len());
    assert_eq!(buffer.anchor, 0);
}

#[test]
fn moves_step_whole_characters() {
    let buffer = at("aé😀b", 0);
    assert_eq!(buffer.next(1), 3);
    assert_eq!(buffer.next(3), 7);
    assert_eq!(buffer.previous(7), 3);
    assert_eq!(buffer.previous(0), 0);
}

#[test]
fn deleting_back_over_a_wide_character_removes_it_whole() {
    let mut buffer = at("a😀", 5);
    let to = buffer.previous(buffer.cursor);
    buffer.delete_to(to);
    assert_eq!(buffer.text(), "a");
}

#[test]
fn utf16_offsets_round_trip() {
    let buffer = Buffer::new("a😀b".to_owned());
    assert_eq!(buffer.to_utf16(5), 3);
    assert_eq!(buffer.offset_from_utf16(3), 5);
    assert_eq!(buffer.offset_from_utf16(99), 6);
    assert_eq!(buffer.range_from_utf16(1..4), 1..6);
}

#[test]
fn composition_marks_text_until_committed() {
    let mut buffer = at("ab", 1);
    buffer.replace(None, "ˆ", true, None);
    assert_eq!(buffer.text(), "aˆb");
    assert_eq!(buffer.marked, Some(1..3));
    buffer.replace(None, "â", false, None);
    assert_eq!(buffer.text(), "âb".replace('â', "aâ").as_str());
    assert_eq!(buffer.marked, None);
    assert_eq!(buffer.cursor, 3);
}

#[test]
fn double_click_selects_the_word() {
    let buffer = Buffer::new("let foo_bar = 1;".to_owned());
    assert_eq!(buffer.word_at(6), 4..11);
    assert_eq!(buffer.word_at(12), 12..13);
}

#[test]
fn reset_forgets_history_and_keeps_the_cursor_inside() {
    let mut buffer = at("abcdef", 6);
    buffer.insert("g");
    buffer.reset("ab".to_owned());
    assert!(!buffer.undo());
    assert_eq!(buffer.cursor, 2);
}
