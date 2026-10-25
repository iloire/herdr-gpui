//! The platform input API's side of the editor: typed text and input
//! method composition arrive here, in UTF-16 offsets over the whole text,
//! and the candidate window is placed from the lines as last drawn.

use super::CodeEditor;
use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, point};
use std::ops::Range;

impl CodeEditor {
    /// The text offset at `position`, from the line drawn there, or the
    /// nearest line's end above or below the ones drawn.
    pub(super) fn offset_at(&self, position: Point<Pixels>) -> Option<usize> {
        let (line, painted) = self
            .painted
            .iter()
            .find(|(_, painted)| {
                position.y >= painted.bounds.top() && position.y < painted.bounds.bottom()
            })
            .or_else(|| {
                // Past the drawn lines: the nearest one.
                self.painted.iter().min_by_key(|(_, painted)| {
                    let middle = painted.bounds.center().y;
                    (f32::from(position.y - middle)).abs() as u32
                })
            })?;
        let drawn = painted
            .layout
            .closest_index_for_x(position.x - painted.bounds.left());
        Some(self.buffer.line_range(*line).start + painted.shown.offset(drawn))
    }
}

impl EntityInputHandler for CodeEditor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.buffer.range_from_utf16(range);
        *actual = Some(self.buffer.to_utf16(range.start)..self.buffer.to_utf16(range.end));
        Some(self.buffer.text()[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let selection = self.buffer.selection();
        Some(UTF16Selection {
            range: self.buffer.to_utf16(selection.start)..self.buffer.to_utf16(selection.end),
            reversed: self.buffer.cursor < self.buffer.anchor,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let marked = self.buffer.marked.clone()?;
        Some(self.buffer.to_utf16(marked.start)..self.buffer.to_utf16(marked.end))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.base.is_none() {
            return;
        }
        let range = range.map(|range| self.buffer.range_from_utf16(range));
        let changed = self.buffer.replace(range, text, false, None);
        self.edited(changed, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.base.is_none() {
            return;
        }
        let range = range.map(|range| self.buffer.range_from_utf16(range));
        // The selection is in UTF-16 units of `text` itself.
        let selected = selected.map(|selected| {
            let byte = |units: usize| {
                let mut seen = 0;
                for (at, c) in text.char_indices() {
                    if seen >= units {
                        return at;
                    }
                    seen += c.len_utf16();
                }
                text.len()
            };
            byte(selected.start)..byte(selected.end)
        });
        let changed = self.buffer.replace(range, text, true, selected);
        self.edited(changed, cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.buffer.range_from_utf16(range);
        let line = self.buffer.line_of(range.start);
        let painted = self.painted.get(&line)?;
        let start = self.buffer.line_range(line).start;
        let x = |offset: usize| {
            let within = offset.clamp(start, self.buffer.line_range(line).end) - start;
            painted.bounds.left() + painted.layout.x_for_index(painted.shown.drawn(within))
        };
        Some(Bounds::from_corners(
            point(x(range.start), painted.bounds.top()),
            point(x(range.end).max(x(range.start)), painted.bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let offset = self.offset_at(position)?;
        Some(self.buffer.to_utf16(offset))
    }
}
