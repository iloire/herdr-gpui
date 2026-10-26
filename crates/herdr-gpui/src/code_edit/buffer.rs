//! The text being edited, its selection, and its history. Offsets are UTF-8
//! byte offsets on character boundaries; only the platform input API speaks
//! UTF-16, converted at its edge. Lines end with `\n` alone: a file read
//! with `\r\n` endings is turned to `\n` on load and back on save
//! (`super::file`).

use std::ops::Range;

/// The most edits kept to undo.
const MAX_UNDO: usize = 500;

/// One change, as undo reverses it: `removed` was at `at` and `inserted`
/// took its place.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Edit {
    at: usize,
    removed: String,
    inserted: String,
    before: (usize, usize),
    after: (usize, usize),
}

#[derive(Debug, Default)]
pub(crate) struct Buffer {
    text: String,
    /// Where each line starts.
    starts: Vec<usize>,
    pub(crate) anchor: usize,
    pub(crate) cursor: usize,
    /// Text an input method is composing, shown underlined until committed.
    pub(crate) marked: Option<Range<usize>>,
    /// The column, in characters, that up and down keep to across shorter
    /// lines; cleared by any other move.
    column: Option<usize>,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// Whether the next typed character starts a new undo step.
    sealed: bool,
}

impl Buffer {
    pub(crate) fn new(text: String) -> Self {
        let mut buffer = Self {
            text,
            sealed: true,
            ..Self::default()
        };
        buffer.index_lines();
        buffer
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    fn index_lines(&mut self) {
        self.starts.clear();
        self.starts.push(0);
        self.starts
            .extend(self.text.match_indices('\n').map(|(at, _)| at + 1));
    }

    pub(crate) fn line_count(&self) -> usize {
        self.starts.len()
    }

    /// Line `line`'s byte range, without its `\n`.
    pub(crate) fn line_range(&self, line: usize) -> Range<usize> {
        let Some(&start) = self.starts.get(line) else {
            return self.text.len()..self.text.len();
        };
        let end = self
            .starts
            .get(line + 1)
            .map_or(self.text.len(), |next| next - 1);
        start..end
    }

    pub(crate) fn line(&self, line: usize) -> &str {
        &self.text[self.line_range(line)]
    }

    /// The line holding `offset`.
    pub(crate) fn line_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|start| *start <= offset) - 1
    }

    pub(crate) fn selection(&self) -> Range<usize> {
        self.anchor.min(self.cursor)..self.anchor.max(self.cursor)
    }

    pub(crate) fn selected(&self) -> &str {
        &self.text[self.selection()]
    }

    /// Moves the cursor to `offset`, keeping the anchor when `extend`.
    pub(crate) fn select_to(&mut self, offset: usize, extend: bool) {
        self.cursor = self.boundary(offset.min(self.text.len()));
        if !extend {
            self.anchor = self.cursor;
        }
        self.column = None;
        self.sealed = true;
    }

    pub(crate) fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.text.len();
        self.sealed = true;
    }

    /// Selects `range` with the cursor at its end.
    pub(crate) fn select(&mut self, range: Range<usize>) {
        self.anchor = self.boundary(range.start.min(self.text.len()));
        self.cursor = self.boundary(range.end.min(self.text.len()));
        self.column = None;
        self.sealed = true;
    }

    /// `offset` moved back to a character boundary.
    fn boundary(&self, mut offset: usize) -> usize {
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    pub(crate) fn previous(&self, offset: usize) -> usize {
        self.text[..offset]
            .char_indices()
            .next_back()
            .map_or(0, |(at, _)| at)
    }

    pub(crate) fn next(&self, offset: usize) -> usize {
        self.text[offset..]
            .chars()
            .next()
            .map_or(offset, |c| offset + c.len_utf8())
    }

    /// The offset `lines` lines above (negative) or below the cursor, at the
    /// column the cursor keeps.
    pub(crate) fn vertical(&mut self, lines: isize) -> usize {
        let line = self.line_of(self.cursor);
        let start = self.line_range(line).start;
        let column = *self
            .column
            .get_or_insert_with(|| self.text[start..self.cursor].chars().count());
        let target = line.saturating_add_signed(lines);
        if lines < 0 && line == 0 {
            return 0;
        }
        if target >= self.line_count() {
            return self.text.len();
        }
        let range = self.line_range(target);
        let text = &self.text[range.clone()];
        range.start
            + text
                .char_indices()
                .nth(column)
                .map_or(text.len(), |(at, _)| at)
    }

    /// Moves vertically, keeping the column for the next vertical move.
    pub(crate) fn move_vertical(&mut self, lines: isize, extend: bool) {
        let offset = self.vertical(lines);
        let column = self.column;
        self.select_to(offset, extend);
        self.column = column;
    }

    /// The start of the line holding `offset`.
    pub(crate) fn line_start(&self, offset: usize) -> usize {
        self.line_range(self.line_of(offset)).start
    }

    /// The end of the line holding `offset`, before its `\n`.
    pub(crate) fn line_end(&self, offset: usize) -> usize {
        self.line_range(self.line_of(offset)).end
    }

    /// The word around `offset`, or the character there when it is none.
    pub(crate) fn word_at(&self, offset: usize) -> Range<usize> {
        let word = |c: char| c.is_alphanumeric() || c == '_';
        let start = self.text[..offset]
            .char_indices()
            .rev()
            .take_while(|(_, c)| word(*c))
            .last()
            .map_or(offset, |(at, _)| at);
        let end = self.text[offset..]
            .char_indices()
            .take_while(|(_, c)| word(*c))
            .last()
            .map_or(offset, |(at, c)| offset + at + c.len_utf8());
        if start == end {
            offset..self.next(offset)
        } else {
            start..end
        }
    }

    /// Replaces `range`, or the composed text, or the selection, with
    /// `text`, as the platform input API asks: `mark` keeps `text` composed,
    /// with `selected` (relative to it) selected. Whether the text changed.
    pub(crate) fn replace(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        mark: bool,
        selected: Option<Range<usize>>,
    ) -> bool {
        let range = range
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection());
        let range = self.boundary(range.start.min(self.text.len()))
            ..self.boundary(range.end.min(self.text.len()));
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let changed = self.splice(range.clone(), &text);
        let end = range.start + text.len();
        if mark && !text.is_empty() {
            self.marked = Some(range.start..end);
            let selected = selected.map_or(end..end, |selected| {
                range.start + selected.start.min(text.len())
                    ..range.start + selected.end.min(text.len())
            });
            self.anchor = self.boundary(selected.start);
            self.cursor = self.boundary(selected.end);
        } else {
            self.marked = None;
            self.anchor = end;
            self.cursor = end;
        }
        self.column = None;
        changed
    }

    /// Replaces the selection with `text`, as typing and pasting do.
    pub(crate) fn insert(&mut self, text: &str) -> bool {
        self.replace(None, text, false, None)
    }

    /// Deletes the selection, or from the cursor to `to` when it is empty.
    pub(crate) fn delete_to(&mut self, to: usize) -> bool {
        let selection = self.selection();
        // Undo puts the cursor back where it was, not a selection of what
        // was deleted.
        let range = if selection.is_empty() {
            to.min(self.cursor)..to.max(self.cursor)
        } else {
            selection
        };
        self.sealed = true;
        let changed = self.replace(Some(range), "", false, None);
        self.sealed = true;
        changed
    }

    /// A new line keeping the current line's indent.
    pub(crate) fn newline(&mut self) -> bool {
        let line = self.line(self.line_of(self.selection().start));
        let indent: String = line
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        self.sealed = true;
        let changed = self.insert(&format!("\n{indent}"));
        self.sealed = true;
        changed
    }

    /// Swaps `range` for `text`, recording it to undo. Typing joins the step
    /// before it until a move or another kind of edit seals that step.
    fn splice(&mut self, range: Range<usize>, text: &str) -> bool {
        if range.is_empty() && text.is_empty() {
            return false;
        }
        let removed = self.text[range.clone()].to_owned();
        if removed == text {
            return false;
        }
        let before = (self.anchor, self.cursor);
        self.text.replace_range(range.clone(), text);
        self.index_lines();
        let after = (range.start + text.len(), range.start + text.len());
        self.redo.clear();
        let typing = removed.is_empty() && !text.contains('\n') && !text.contains(' ');
        match self.undo.last_mut() {
            Some(last)
                if typing
                    && !self.sealed
                    && last.removed.is_empty()
                    && last.at + last.inserted.len() == range.start =>
            {
                last.inserted.push_str(text);
                last.after = after;
            }
            _ => {
                self.undo.push(Edit {
                    at: range.start,
                    removed,
                    inserted: text.to_owned(),
                    before,
                    after,
                });
                if self.undo.len() > MAX_UNDO {
                    self.undo.remove(0);
                }
            }
        }
        self.sealed = !typing;
        true
    }

    /// Takes back the last step; whether there was one.
    pub(crate) fn undo(&mut self) -> bool {
        let Some(edit) = self.undo.pop() else {
            return false;
        };
        self.text
            .replace_range(edit.at..edit.at + edit.inserted.len(), &edit.removed);
        self.index_lines();
        (self.anchor, self.cursor) = edit.before;
        self.redo.push(edit);
        self.after_history();
        true
    }

    /// Makes the last step undone again; whether there was one.
    pub(crate) fn redo(&mut self) -> bool {
        let Some(edit) = self.redo.pop() else {
            return false;
        };
        self.text
            .replace_range(edit.at..edit.at + edit.removed.len(), &edit.inserted);
        self.index_lines();
        (self.anchor, self.cursor) = edit.after;
        self.undo.push(edit);
        self.after_history();
        true
    }

    fn after_history(&mut self) {
        self.marked = None;
        self.column = None;
        self.sealed = true;
        self.anchor = self.boundary(self.anchor.min(self.text.len()));
        self.cursor = self.boundary(self.cursor.min(self.text.len()));
    }

    /// Replaces the whole text with `text`, as taking the file's version
    /// from disk does: one step undo takes back, keeping the cursor on the
    /// same line and column where the new text has them. Only the stretch
    /// that differs is replaced, so undo restores just that.
    pub(crate) fn replace_all(&mut self, text: &str) -> bool {
        let old = self.text.as_str();
        let prefix = old
            .char_indices()
            .zip(text.chars())
            .take_while(|((_, a), b)| a == b)
            .last()
            .map_or(0, |((at, c), _)| at + c.len_utf8());
        let suffix = old[prefix..]
            .chars()
            .rev()
            .zip(text[prefix..].chars().rev())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        let line = self.line_of(self.cursor);
        let column = self.text[self.line_range(line).start..self.cursor]
            .chars()
            .count();
        let range = prefix..old.len() - suffix;
        let inserted = &text[prefix..text.len() - suffix];
        self.sealed = true;
        let changed = self.splice(range, inserted);
        self.sealed = true;
        let line = line.min(self.line_count() - 1);
        let within = self.line(line);
        let at = self.line_range(line).start
            + within
                .char_indices()
                .nth(column)
                .map_or(within.len(), |(at, _)| at);
        self.marked = None;
        self.select_to(at, false);
        changed
    }

    /// Replaces the whole text, as first reading the file does, forgetting
    /// its history and keeping the cursor near where it was.
    pub(crate) fn reset(&mut self, text: String) {
        let cursor = self.cursor;
        *self = Self::new(text);
        let at = self.boundary(cursor.min(self.text.len()));
        self.select_to(at, false);
    }

    pub(crate) fn to_utf16(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .encode_utf16()
            .count()
    }

    pub(crate) fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut units = 0;
        for (byte, c) in self.text.char_indices() {
            if units >= offset {
                return byte;
            }
            units += c.len_utf16();
        }
        self.text.len()
    }

    pub(crate) fn range_from_utf16(&self, range: Range<usize>) -> Range<usize> {
        let (start, end) = (
            self.offset_from_utf16(range.start),
            self.offset_from_utf16(range.end),
        );
        start.min(end)..start.max(end)
    }
}
