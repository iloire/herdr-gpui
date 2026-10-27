//! The field under the Files header that narrows the tree to the files
//! whose names hold what is typed, with every folder on the way to one
//! open. Enter opens the first match, Escape clears the field and gives
//! the keyboard back to the terminal. Matching walks the listing already
//! read, so typing never touches the disk.

use crate::{
    HerdrWindow,
    search_input::{Changed, SearchInput},
};
use gpui::{prelude::*, *};

use super::{Opening, tree::Entry};

/// The field and its subscription, made the first time the section shows.
pub(super) struct Filter {
    input: Entity<SearchInput>,
    _changed: Subscription,
}

impl HerdrWindow {
    /// Gives the section its field, in the sidebar's face.
    pub(super) fn sync_files_filter(&mut self, cx: &mut Context<Self>) {
        let font = self.config.sidebar.clone();
        let theme = self.theme.clone();
        if let Some(filter) = &self.files.filter {
            filter
                .input
                .update(cx, |input, cx| input.set_appearance(font, theme, cx));
            return;
        }
        let input = cx.new(|cx| {
            let mut input = SearchInput::new(cx);
            input.set_placeholder("Filter files", cx);
            input.set_appearance(font, theme, cx);
            input.set_frameless(cx);
            input
        });
        let changed = cx.subscribe(&input, |this, input, _: &Changed, cx| {
            this.files.set_query(input.read(cx).text());
            this.files.scroll.scroll_to_item(0, ScrollStrategy::Top);
            cx.notify();
        });
        self.files.filter = Some(Filter {
            input,
            _changed: changed,
        });
        cx.notify();
    }

    /// The filter's row under the header, once the field exists.
    pub(super) fn files_filter_field(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let input = self.files.filter.as_ref()?.input.clone();
        let theme = &self.theme;
        let focus = input.read(cx).focus.clone();
        let typed = self.files.filtering();
        Some(
            div()
                .id("files-filter")
                .debug_selector(|| "files-filter".into())
                .flex_none()
                .px_3()
                .py_1()
                .flex()
                .items_center()
                .gap(px(6.))
                .border_b_1()
                .border_color(rgb(theme.active))
                .cursor(CursorStyle::IBeam)
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    window.focus(&focus, cx);
                })
                .on_key_down(cx.listener(Self::files_filter_key))
                .child(
                    svg()
                        .path("icons/search.svg")
                        .size(px(12.))
                        .flex_none()
                        .text_color(rgb(theme.muted)),
                )
                .child(div().flex_1().min_w_0().child(input))
                .when(typed, |field| {
                    field.child(
                        div()
                            .id("files-filter-clear")
                            .debug_selector(|| "files-filter-clear".into())
                            .flex_none()
                            .p(px(3.))
                            .rounded_sm()
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(theme.active)))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.clear_files_filter(window, cx);
                            }))
                            .child(
                                svg()
                                    .path("icons/x.svg")
                                    .size(px(12.))
                                    .text_color(rgb(theme.muted)),
                            ),
                    )
                }),
        )
    }

    /// Escape clears the field; Enter opens the first file it matches.
    fn files_filter_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(filter) = &self.files.filter else {
            return;
        };
        if filter.input.read(cx).is_composing() {
            return;
        }
        match event.keystroke.key.as_str() {
            "escape" => self.clear_files_filter(window, cx),
            "enter" => {
                let Some(path) = self.files_first_match() else {
                    return;
                };
                let opening = if event.keystroke.modifiers.secondary() {
                    Opening::Editor
                } else {
                    Opening::Tab
                };
                self.open_files_entry(&path, opening, window, cx);
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    /// The first file the filter shows, relative to its checkout.
    fn files_first_match(&self) -> Option<String> {
        if !self.files.filtering() {
            return None;
        }
        let tree = &self.files.listing.as_ref()?.tree;
        self.files
            .rows
            .iter()
            .filter_map(|row| tree.node(row.node))
            .find(|node| matches!(node.entry, Entry::File(_)))
            .map(|node| node.path.clone())
    }

    /// Empties the field and gives the keyboard back to the terminal.
    fn clear_files_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(filter) = &self.files.filter {
            filter.input.update(cx, |input, cx| input.clear(cx));
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }
}
