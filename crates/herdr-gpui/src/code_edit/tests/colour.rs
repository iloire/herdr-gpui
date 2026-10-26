use crate::{
    code_edit::{
        colour::{Segment, segments},
        display::Shown,
    },
    review::highlight::{Span, Token},
};

fn span(start: usize, end: usize, token: Token) -> Span {
    Span { start, end, token }
}

#[test]
fn spans_colour_their_stretch_of_the_drawn_line() {
    let line = "\tlet x";
    let shown = Shown::of(line);
    let cut = segments(line, &shown, &[span(1, 4, Token::Keyword)], None);
    assert_eq!(
        cut,
        [
            Segment {
                range: 0..4,
                token: None,
                marked: false
            },
            Segment {
                range: 4..7,
                token: Some(Token::Keyword),
                marked: false
            },
            Segment {
                range: 7..9,
                token: None,
                marked: false
            },
        ]
    );
}

#[test]
fn stale_spans_off_the_line_are_dropped() {
    let line = "é";
    let shown = Shown::of(line);
    let cut = segments(
        line,
        &shown,
        &[span(1, 2, Token::String), span(0, 9, Token::Type)],
        None,
    );
    assert_eq!(
        cut,
        [Segment {
            range: 0..2,
            token: None,
            marked: false
        }]
    );
}

#[test]
fn composed_text_is_its_own_segment() {
    let line = "abcd";
    let shown = Shown::of(line);
    let cut = segments(line, &shown, &[span(0, 4, Token::Comment)], Some(1..3));
    let marked: Vec<_> = cut
        .iter()
        .map(|segment| (segment.range.clone(), segment.marked))
        .collect();
    assert_eq!(marked, [(0..1, false), (1..3, true), (3..4, false)]);
    assert!(
        cut.iter()
            .all(|segment| segment.token == Some(Token::Comment))
    );
}
