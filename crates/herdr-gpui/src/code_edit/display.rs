//! What a line looks like on screen, which is not quite its text: a tab
//! widens to the next stop, and a control or direction-override character
//! shows as a replacement mark, so what is drawn is what is there and no
//! hidden character reorders the line. Editing keeps the real characters;
//! this maps offsets between the two.

use herdr_client::text::unsafe_char;

/// Columns between tab stops.
const TAB: usize = 4;
/// Drawn for a character that must not be drawn as itself.
const MARK: char = '\u{fffd}';

/// One line as drawn.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Shown {
    pub(crate) text: String,
    /// The drawn offset of each character boundary of the line, in order,
    /// as (line offset, drawn offset), ending with the line's end.
    marks: Vec<(usize, usize)>,
}

impl Shown {
    pub(crate) fn of(line: &str) -> Self {
        let mut text = String::with_capacity(line.len());
        let mut marks = Vec::with_capacity(line.len() + 1);
        let mut column = 0;
        for (at, c) in line.char_indices() {
            marks.push((at, text.len()));
            if c == '\t' {
                let width = TAB - column % TAB;
                text.extend(std::iter::repeat_n(' ', width));
                column += width;
            } else if unsafe_char(c) {
                text.push(MARK);
                column += 1;
            } else {
                text.push(c);
                column += 1;
            }
        }
        marks.push((line.len(), text.len()));
        Self { text, marks }
    }

    /// Where the line's `offset` is drawn.
    pub(crate) fn drawn(&self, offset: usize) -> usize {
        let index = self.marks.partition_point(|(at, _)| *at <= offset);
        self.marks[index.saturating_sub(1)].1
    }

    /// The line offset drawn at or just before `drawn`: a click inside a
    /// widened tab lands before it.
    pub(crate) fn offset(&self, drawn: usize) -> usize {
        let index = self.marks.partition_point(|(_, at)| *at <= drawn);
        self.marks[index.saturating_sub(1)].0
    }
}
