//! Drawing the editor: line numbers, then each line in view as drawn text
//! (`display`) in its syntax colours (`colour`), with the current line
//! tinted, its part of the selection, the caret, and composed text
//! underlined. Only the lines in view are laid out. Each records where it
//! was drawn, which the mouse and the input method read back (`input`).

use super::{CodeEditor, Painted, colour::segments, display::Shown};
use crate::{actions, fonts::StyledFont, input::ViewInputHandler, review::token_colour};
use gpui::{
    AnyElement, Bounds, ContentMask, Context, CursorStyle, ListHorizontalSizingBehavior,
    MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Rgba, TextAlign, TextRun, UnderlineStyle,
    Window, canvas, div, fill, point, prelude::*, px, rgb, rgba, size, uniform_list,
};
use std::ops::Range;

/// A character's width as a share of the font size, to size lines for
/// sideways scrolling before they are laid out.
const ADVANCE: f32 = 0.62;
/// Room kept between the caret and the side of the view it scrolls to.
const MARGIN: f32 = 24.;

/// One line's share of what is drawn over its text.
struct Marks {
    /// The selected part, in drawn offsets, and whether it runs on past the
    /// line's end.
    selected: Option<(Range<usize>, bool)>,
    caret: Option<usize>,
    /// Whether this is the cursor's line, tinted while nothing is selected.
    current: bool,
}

impl CodeEditor {
    /// What of the selection and caret falls on `line`, in the drawn
    /// offsets of `shown`.
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
        let on_line = self.buffer.line_of(self.buffer.cursor) == line;
        Marks {
            selected,
            caret: (focused && on_line).then(|| local(self.buffer.cursor)),
            current: on_line && selection.is_empty(),
        }
    }

    /// Line `line`'s runs: its length, colour, and whether it is composed.
    fn runs(&self, line: usize, shown: &Shown) -> Vec<(usize, Rgba, bool)> {
        let text = self.buffer.line(line);
        let start = self.buffer.line_range(line).start;
        let marked = self.buffer.marked.clone().map(|marked| {
            marked.start.saturating_sub(start).min(text.len())
                ..marked.end.saturating_sub(start).min(text.len())
        });
        let spans = self.spans.get(line).map_or(&[][..], Vec::as_slice);
        let theme = &self.look.theme;
        segments(text, shown, spans, marked)
            .into_iter()
            .map(|segment| {
                let colour = segment
                    .token
                    .map_or(rgb(theme.foreground), |token| token_colour(theme, token));
                (segment.range.len(), colour, segment.marked)
            })
            .collect()
    }

    fn row(&self, line: usize, gutter: f32, focused: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.look.theme;
        let height = px(self.look.line_height);
        let shown = Shown::of(self.buffer.line(line));
        let marks = self.marks(line, &shown, focused);
        let runs = self.runs(line, &shown);
        let width = (shown.text.chars().count() + 2) as f32 * self.look.font.size * ADVANCE;
        let drawn = shown.text.clone();
        let (selection, cursor) = (rgb(theme.active), rgb(theme.cursor));
        let current = rgba((theme.active << 8) | 0x66);
        let number = rgb(if marks.current {
            theme.foreground
        } else {
            theme.muted
        });
        let editor = cx.entity();
        div()
            .id(line)
            .debug_selector(move || format!("code-line-{line}"))
            .h(height)
            .flex()
            .items_center()
            .min_w_full()
            .whitespace_nowrap()
            .when(marks.current, |row| row.bg(current))
            .child(
                div()
                    .flex_none()
                    .w(px(gutter))
                    .pr(px(12.))
                    .flex()
                    .justify_end()
                    .text_color(number)
                    .child((line + 1).to_string()),
            )
            .child(
                canvas(
                    move |_, window, _| {
                        let font = window.text_style().font();
                        let runs: Vec<TextRun> = runs
                            .iter()
                            .map(|(len, colour, marked)| TextRun {
                                len: *len,
                                font: font.clone(),
                                color: (*colour).into(),
                                background_color: None,
                                underline: marked.then(|| UnderlineStyle {
                                    color: Some((*colour).into()),
                                    thickness: px(1.),
                                    wavy: false,
                                }),
                                strikethrough: None,
                            })
                            .collect();
                        let size = window.text_style().font_size.to_pixels(window.rem_size());
                        let layout =
                            window
                                .text_system()
                                .shape_line(drawn.into(), size, &runs, None);
                        (layout, marks)
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
                                    selection,
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
                                    cursor,
                                ));
                            }
                        });
                        let caret = marks
                            .caret
                            .map(|caret| bounds.left() + layout.x_for_index(caret));
                        let scrolled = editor.update(cx, |editor, _| {
                            editor.painted.insert(
                                line,
                                Painted {
                                    bounds,
                                    shown,
                                    layout,
                                },
                            );
                            caret.is_some_and(|caret| editor.follow_caret(caret))
                        });
                        if scrolled {
                            window.refresh();
                        }
                    },
                )
                .w(px(width))
                .h(height),
            )
            .into_any_element()
    }

    /// Scrolls sideways so the caret, drawn at `x`, is in view, after an
    /// edit or move asked for it. Whether it scrolled.
    fn follow_caret(&mut self, x: Pixels) -> bool {
        if !std::mem::take(&mut self.follow) {
            return false;
        }
        let Some(viewport) = self.viewport else {
            return false;
        };
        let handle = self.scroll.0.borrow().base_handle.clone();
        let offset = handle.offset();
        let shift = if x > viewport.right() - px(MARGIN) {
            viewport.right() - px(MARGIN) - x
        } else if x < viewport.left() + px(MARGIN) && offset.x < px(0.) {
            (viewport.left() + px(MARGIN) - x).min(-offset.x)
        } else {
            return false;
        };
        handle.set_offset(point((offset.x + shift).min(px(0.)), offset.y));
        true
    }
}

impl Render for CodeEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.painted.clear();
        let font = self.look.font.clone();
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
            .line_height(px(self.look.line_height))
            .text_color(rgb(self.look.theme.foreground))
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
                        editor.update(cx, |editor, _| editor.viewport = Some(bounds));
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}
