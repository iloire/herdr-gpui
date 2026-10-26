//! A code tab draws its editor beside the file's outline, under a header
//! naming the file; reading and editing live elsewhere.

use super::{CodeView, Outline};
use crate::{
    HerdrWindow,
    browser::{Slot, Tab, TabId},
};
use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};

/// The outline's share of the tab, so a narrow group keeps room for code.
const OUTLINE_WIDTH: f32 = 240.;
/// Below this width the outline stays hidden whatever its toggle says.
const OUTLINE_MIN_TAB_WIDTH: f32 = 640.;

/// Records a tab's width as it lays out, and draws again when that moves it
/// across the outline's threshold, so the outline follows on the next frame.
fn measure(width: Rc<Cell<f32>>) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let now = f32::from(bounds.size.width);
            let before = width.replace(now);
            if (before >= OUTLINE_MIN_TAB_WIDTH) != (now >= OUTLINE_MIN_TAB_WIDTH) {
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

impl HerdrWindow {
    pub(super) fn code_line_height(&self) -> f32 {
        self.config.terminal.line_height().max(14.)
    }

    fn code_outline(&self, id: TabId, outline: &Outline, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        div()
            .flex_none()
            .w(px(OUTLINE_WIDTH))
            .border_l_1()
            .border_color(rgb(theme.active))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .px(px(10.))
                    .py(px(6.))
                    .text_color(rgb(theme.muted))
                    .child(format!("Outline \u{00b7} {}", outline.entries.len())),
            )
            .child(
                uniform_list(
                    "code-outline",
                    outline.entries.len(),
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        let Some(outline) = this
                            .code_views
                            .get(&id)
                            .and_then(|view| view.outline.clone())
                        else {
                            return Vec::new();
                        };
                        range
                            .filter_map(|index| {
                                let entry = outline.entries.get(index)?;
                                let line = entry.line;
                                Some(
                                    div()
                                        .id(index)
                                        .debug_selector(move || format!("code-outline-{index}"))
                                        .px(px(10.))
                                        .py(px(2.))
                                        .flex()
                                        .gap(px(8.))
                                        .cursor_pointer()
                                        .hover(|row| row.bg(rgb(this.theme.active)))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .child(entry.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .flex_none()
                                                .text_color(rgb(this.theme.muted))
                                                .child(entry.kind.label()),
                                        )
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            if let Some(view) = this.code_views.get(&id) {
                                                let focus = view.focus.clone();
                                                view.editor.update(cx, |editor, cx| {
                                                    editor.go_to_line(line, cx);
                                                });
                                                window.focus(&focus, cx);
                                            }
                                        })),
                                )
                            })
                            .collect()
                    }),
                )
                .flex_1()
                .min_h_0(),
            )
    }

    fn code_header(&self, id: TabId, tab: &Tab, view: &CodeView, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        let path = tab
            .location
            .as_ref()
            .map(crate::browser::Location::display)
            .unwrap_or_default();
        let button = |label: &'static str, selector: &'static str| {
            div()
                .id(selector)
                .debug_selector(move || selector.into())
                .flex_none()
                .px_2()
                .py_1()
                .rounded(px(crate::config::corners::CONTROL))
                .cursor_pointer()
                .hover(|button| button.bg(rgb(theme.active)))
                .child(label)
        };
        div()
            .flex_none()
            .px(px(10.))
            .py(px(6.))
            .flex()
            .items_center()
            .gap(px(8.))
            .border_b_1()
            .border_color(rgb(theme.active))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(rgb(theme.muted))
                    .child(path),
            )
            .children(self.code_edit_header(id, view, cx))
            .child(
                button(
                    if view.show_outline {
                        "Hide Outline"
                    } else {
                        "Outline"
                    },
                    "code-outline-toggle",
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(view) = this.code_views.get_mut(&id) {
                        view.show_outline = !view.show_outline;
                        cx.notify();
                    }
                })),
            )
            .when(self.markdown_toggles(id, cx), |header| {
                header.child(button("Preview", "code-markdown-preview").on_click(
                    cx.listener(move |this, _, window, cx| this.toggle_markdown(id, window, cx)),
                ))
            })
            .child(
                button("Open in Editor", "code-open-editor").on_click(
                    cx.listener(move |this, _, _, cx| this.open_code_view_in_editor(id, cx)),
                ),
            )
    }

    /// A code tab, drawn in `slot` where a page would be.
    pub(crate) fn render_code_tab(
        &self,
        slot: Slot,
        tab: &Tab,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = tab.id;
        let Some(view) = self.code_views.get(&id) else {
            return div()
                .id(SharedString::from(slot.selector("code-loading")))
                .size_full()
                .into_any_element();
        };
        let look = self.code_look();
        view.editor
            .update(cx, |editor, cx| editor.set_look(look, cx));
        let outline = view
            .outline
            .as_ref()
            .filter(|outline| {
                view.show_outline
                    && !outline.entries.is_empty()
                    && view.width.get() >= OUTLINE_MIN_TAB_WIDTH
            })
            .map(|outline| self.code_outline(id, outline, cx));
        div()
            .id(SharedString::from(slot.selector("code-tab")))
            .debug_selector(|| "code-view".into())
            .relative()
            .size_full()
            .min_w_0()
            .flex()
            .flex_col()
            .bg(rgb(self.theme.background))
            .child(measure(view.width.clone()))
            .child(self.code_header(id, tab, view, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(div().flex_1().min_w_0().child(view.editor.clone()))
                    .children(outline),
            )
            .into_any_element()
    }
}
