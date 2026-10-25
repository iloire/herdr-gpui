//! Drawing the editor: line numbers, then each line in view as drawn text
//! (`display`) with its part of the selection, the caret, and composed text
//! underlined. Only the lines in view are laid out. Each records where it
//! was drawn, which the mouse and the input method read back (`input`).

use super::{CodeEditor, Painted, display::Shown};
use crate::{actions, fonts::StyledFont, input::ViewInputHandler};
use gpui::{
    AnyElement, Bounds, ContentMask, Context, CursorStyle, ListHorizontalSizingBehavior,
    MouseButton, MouseDownEvent, MouseMoveEvent, TextAlign, TextRun, UnderlineStyle, Window,
    canvas, div, fill, point, prelude::*, px, rgb, size, uniform_list,
};
use std::ops::Range;

/// A character's width as a share of the font size, to size lines for
/// sideways scrolling before they are laid out.
const ADVANCE: f32 = 0.62;

/// One line's share of what is drawn over its text.
struct Marks {
    /// The selected part, in drawn offsets, and whether it runs on past the
    /// line's end.
    selected: Option<(Range<usize>, bool)>,
    caret: Option<usize>,
    marked: Option<Range<usize>>,
}

impl CodeEditor {
    /// What of the selection, caret, and composition falls on `line`, in
    /// the drawn offsets of `shown`.
    fn marks(&self, line: usize, shown: &Shown, focused: bool) -> Marks {
        let range = self.buffer.line_range(line);
        let local = |offset: usize| shown.drawn(offset.clamp(range.start, range.end) - range.start);
        let selection = self.buffer.selection();
        let selected =
            (!selection.is_empty() && selection.start <= range.end && selection.end > range.start)
                .then(|| {
                    (
                        local(selection.start)..local(selection.end),
                        selection.end > range.end,
                    )
                });
        let caret = (focused && self.buffer.line_of(self.buffer.cursor) == line)
            .then(|| local(self.buffer.cursor));
        let marked = self
            .buffer
            .marked
            .clone()
            .filter(|marked| marked.start <= range.end && marked.end >= range.start)
            .map(|marked| local(marked.start)..local(marked.end));
        Marks {
            selected,
            caret,
            marked,
        }
    }

    fn row(&self, line: usize, gutter: f32, focused: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.theme;
        let height = px(self.line_height);
        let shown = Shown::of(self.buffer.line(line));
        let marks = self.marks(line, &shown, focused);
        let width = (shown.text.chars().count() + 2) as f32 * self.font.size * ADVANCE;
        let text = shown.text.clone();
        let (foreground, selection, cursor) = (theme.foreground, theme.active, theme.cursor);
        let editor = cx.entity();
        div()
            .id(line)
            .h(height)
            .flex()
            .items_center()
            .whitespace_nowrap()
            .child(
                div()
                    .flex_none()
                    .w(px(gutter))
                    .pr(px(12.))
                    .flex()
                    .justify_end()
                    .text_color(rgb(theme.muted))
                    .child((line + 1).to_string()),
            )
            .child(
                canvas(
                    move |_, window, _| {
                        let run = TextRun {
                            len: text.len(),
                            font: window.text_style().font(),
                            color: rgb(foreground).into(),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let runs = match &marks.marked {
                            Some(marked) if !marked.is_empty() => [
                                (0..marked.start, None),
                                (
                                    marked.clone(),
                                    Some(UnderlineStyle {
                                        color: Some(run.color),
                                        thickness: px(1.),
                                        wavy: false,
                                    }),
                                ),
                                (marked.end..text.len(), None),
                            ]
                            .into_iter()
                            .filter(|(range, _)| !range.is_empty())
                            .map(|(range, underline)| TextRun {
                                len: range.len(),
                                underline,
                                ..run.clone()
                            })
                            .collect(),
                            _ => vec![run],
                        };
                        let size = window.text_style().font_size.to_pixels(window.rem_size());
                        (
                            window
                                .text_system()
                                .shape_line(text.into(), size, &runs, None),
                            marks,
                        )
                    },
                    move |bounds, (layout, marks), window, cx| {
                        window.with_content_mask(Some(ContentMask { bounds }), |window| {
                            if let Some((selected, past_end)) = &marks.selected {
                                let end = layout.x_for_index(selected.end)
                                    + if *past_end { px(6.) } else { px(0.) };
                                window.paint_quad(fill(
                                    Bounds::from_corners(
                                        point(
                                            bounds.left() + layout.x_for_index(selected.start),
                                            bounds.top(),
                                        ),
                                        point(bounds.left() + end, bounds.bottom()),
                                    ),
                                    rgb(selection),
                                ));
                            }
                            let _ = layout.paint(
                                bounds.origin,
                                height,
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            );
                            if let Some(caret) = marks.caret {
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(
                                            bounds.left() + layout.x_for_index(caret),
                                            bounds.top(),
                                        ),
                                        size(px(1.5), height),
                                    ),
                                    rgb(cursor),
                                ));
                            }
                        });
                        editor.update(cx, |editor, _| {
                            editor.painted.insert(
                                line,
                                Painted {
                                    bounds,
                                    shown,
                                    layout,
                                },
                            );
                        });
                    },
                )
                .w(px(width))
                .h(height),
            )
            .into_any_element()
    }
}

impl Render for CodeEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.painted.clear();
        let font = self.font.clone();
        let count = self.buffer.line_count();
        let digits = count.max(1).ilog10() + 1;
        let gutter = (digits as f32 + 2.) * font.size * ADVANCE;
        let widest = (0..count).max_by_key(|line| self.buffer.line_range(*line).len());
        let focused = self.focus.is_focused(window);
        let editor = cx.entity();
        div()
            .id("code-edit")
            .debug_selector(|| "code-edit".into())
            .role(gpui::Role::TextInput)
            .aria_label("Edit file")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .text_font(&font)
            .text_size(px(font.size))
            .line_height(px(self.line_height))
            .text_color(rgb(self.theme.foreground))
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(Self::key_down))
            .on_action(cx.listener(|this, _: &actions::Cut, window, cx| {
                this.key_down(&actions::edit_key("x"), window, cx);
            }))
            .on_action(cx.listener(|this, _: &actions::Copy, window, cx| {
                this.key_down(&actions::edit_key("c"), window, cx);
            }))
            .on_action(cx.listener(|this, _: &actions::Paste, window, cx| {
                this.key_down(&actions::edit_key("v"), window, cx);
            }))
            .on_action(cx.listener(|this, _: &actions::SelectAll, window, cx| {
                this.key_down(&actions::edit_key("a"), window, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    cx.stop_propagation();
                    if this.buffer.marked.is_some() {
                        return;
                    }
                    let Some(offset) = this.offset_at(event.position) else {
                        return;
                    };
                    match event.click_count {
                        2 => this.buffer.select(this.buffer.word_at(offset)),
                        3.. => {
                            let line = this.buffer.line_of(offset);
                            let range = this.buffer.line_range(line);
                            let end = this.buffer.next(range.end);
                            this.buffer.select(range.start..end);
                        }
                        _ => this.buffer.select_to(offset, event.modifiers.shift),
                    }
                    this.selecting = true;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.selecting
                    && event.pressed_button == Some(MouseButton::Left)
                    && let Some(offset) = this.offset_at(event.position)
                {
                    this.buffer.select_to(offset, true);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .child(
                uniform_list(
                    "code-edit-lines",
                    count,
                    cx.processor(move |this, range: Range<usize>, _, cx| {
                        range
                            .map(|line| this.row(line, gutter, focused, cx))
                            .collect()
                    }),
                )
                .with_width_from_item(widest)
                .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
                .track_scroll(&self.scroll)
                .flex_1()
                .min_h_0(),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        let focus = editor.read(cx).focus.clone();
                        window.handle_input(
                            &focus,
                            ViewInputHandler::new(bounds, editor.clone()),
                            cx,
                        );
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}
