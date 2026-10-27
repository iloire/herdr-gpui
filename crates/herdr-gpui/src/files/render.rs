//! Drawing the Files section: a header naming the checkout and the folder
//! followed, then the tree, its changed files colored. Every row has
//! one height so the list draws only the rows in view, however large the
//! checkout. A file opens in a code tab beside the terminals, a markdown
//! file rendered (see `browser::markdown_toggle`), or with the secondary
//! modifier in the terminal editor beside the focused pane. A right-click
//! opens the row's menu (see `menu`). The filter field sits under the
//! header (see `filter`).

use super::{
    Status,
    tree::{Entry, Row},
};
use crate::{HerdrWindow, editor::EditorTarget, fonts::StyledFont};
use gpui::{prelude::*, *};
use herdr_vcs::code_index::Change;
use std::ops::Range;

/// How far each level of the tree steps in.
const INDENT: f32 = 12.;

/// Where a file from the tree opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Opening {
    /// A code tab, or a markdown file rendered.
    Tab,
    /// A code tab, even for markdown.
    Source,
    /// The terminal editor beside the focused pane.
    Editor,
}

impl HerdrWindow {
    /// The section's column, `width` wide with rows `row_height` tall.
    pub(crate) fn render_files(
        &self,
        width: f32,
        row_height: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = &self.theme;
        let font = &self.config.sidebar;
        let column = div()
            .id("files")
            .debug_selector(|| "files".into())
            .role(Role::Navigation)
            .aria_label("Files")
            .w(px(width))
            .flex_none()
            .h_full()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .text_font(font)
            .text_size(px(font.size - 1.))
            .text_color(rgb(theme.foreground))
            .bg(self.fill().panel(theme.sidebar_background()))
            .border_r_1()
            .border_color(rgb(theme.active))
            .child(self.files_header(cx));
        let message = match &self.files.status {
            Status::Idle => Some("Focus a pane to see the files it works on.".to_owned()),
            Status::Reading => Some("Reading…".to_owned()),
            Status::Remote => Some(
                "Files lists this computer's checkouts; this pane runs on another host.".into(),
            ),
            Status::Failed(error) => Some(format!(
                "No Git checkout to list at {}\n\n{error}",
                self.files.followed().unwrap_or("this folder")
            )),
            Status::Ready if self.files.filtering() && self.files.rows.is_empty() => {
                Some("No file names match.".to_owned())
            }
            Status::Ready => None,
        };
        let column = column.children(
            matches!(self.files.status, Status::Ready)
                .then(|| self.files_filter_field(cx))
                .flatten(),
        );
        if let Some(message) = message {
            return column.child(
                div()
                    .debug_selector(|| "files-message".into())
                    .p_3()
                    .text_color(rgb(theme.muted))
                    .child(message),
            );
        }
        let count = self.files.rows.len();
        column.child(
            uniform_list(
                "files-list",
                count,
                cx.processor(move |this, range: Range<usize>, _, cx| {
                    range
                        .filter_map(|item| this.files_row(item, row_height, cx))
                        .collect()
                }),
            )
            .track_scroll(&self.files.scroll)
            .flex_1()
            .min_h_0()
            .pb_2(),
        )
    }

    fn files_header(&self, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        let name = self
            .files
            .root()
            .and_then(|root| root.file_name())
            .map(|name| name.to_string_lossy().into_owned());
        let pinned = self.files.pinned;
        let (foreground, surface) = (theme.foreground, theme.surface);
        let button = |id: &'static str, icon: &'static str, color: u32, hint: &'static str| {
            div()
                .id(id)
                .debug_selector(move || id.into())
                .flex_none()
                .p(px(3.))
                .rounded_sm()
                .cursor_pointer()
                .hover(|style| style.bg(rgb(theme.active)))
                .child(svg().path(icon).size(px(12.)).text_color(rgb(color)))
                .tooltip(move |_, cx| {
                    cx.new(|_| crate::menu::SettingsHint {
                        text: hint.into(),
                        foreground,
                        surface,
                    })
                    .into()
                })
        };
        div()
            .flex_none()
            .px_3()
            .py_1()
            .flex()
            .items_center()
            .gap_1()
            .border_b_1()
            .border_color(rgb(theme.active))
            .child(
                div()
                    .flex_none()
                    .max_w(relative(0.6))
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .children(name),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(rgb(theme.muted))
                    .children(self.files.here.as_ref().map(|here| format!("/ {here}"))),
            )
            .child(
                button(
                    "files-collapse",
                    "icons/chevron-up.svg",
                    theme.muted,
                    "Collapse folders",
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.files.collapse_all();
                    cx.notify();
                })),
            )
            .child(
                button(
                    "files-pin",
                    "icons/lock.svg",
                    if pinned { theme.primary() } else { theme.muted },
                    if pinned {
                        "Pinned here: follow the focused pane again"
                    } else {
                        "Following the focused pane: pin it here"
                    },
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_files_pin(cx))),
            )
    }

    fn files_row(
        &self,
        item: usize,
        row_height: f32,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let theme = &self.theme;
        let listing = self.files.listing.as_ref()?;
        let Row { node: id, depth } = *self.files.rows.get(item)?;
        let node = listing.tree.node(id)?;
        let (menu, folder) = (node.path.clone(), matches!(node.entry, Entry::Folder(_)));
        let row = div()
            .id(("files-row", item))
            .h(px(row_height))
            .px_2()
            .flex()
            .items_center()
            .gap(px(5.))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.open_files_menu(&menu, folder, event.position, window, cx);
                }),
            );
        let indent = f32::from(depth) * INDENT;
        Some(match node.entry {
            Entry::Folder(_) => {
                let open = self.files.is_open(&node.path);
                let here = self.files.here.as_deref() == Some(node.path.as_str());
                let path = node.path.clone();
                row.debug_selector(|| "files-folder".into())
                    .pl(px(8. + indent))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(theme.active)))
                    .child(
                        svg()
                            .path(if open {
                                "icons/chevron-down.svg"
                            } else {
                                "icons/chevron-right.svg"
                            })
                            .size(px(10.))
                            .flex_none()
                            .text_color(rgb(theme.muted)),
                    )
                    .child(
                        svg()
                            .path("icons/folder.svg")
                            .size(px(12.))
                            .flex_none()
                            .text_color(rgb(if here { theme.primary() } else { theme.muted })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .when(here, |name| {
                                name.text_color(rgb(theme.primary()))
                                    .font_weight(FontWeight::SEMIBOLD)
                            })
                            .child(node.name.clone()),
                    )
                    .when(node.changed > 0, |row| {
                        row.child(
                            div()
                                .flex_none()
                                .text_size(px(self.config.sidebar.size - 2.))
                                .text_color(rgb(theme.ink(theme.palette[3])))
                                .child(node.changed.to_string()),
                        )
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.files.toggle(&path);
                        cx.notify();
                    }))
                    .into_any_element()
            }
            Entry::File(file) => {
                let change = listing.index.change(file);
                // Past the chevron's room, so files line up with
                // their folder's name.
                self.files_file_row(row.pl(px(23. + indent)), &node.path, change, cx)
                    .debug_selector(|| "files-file".into())
                    .child(div().flex_1().min_w_0().truncate().child(node.name.clone()))
                    .child(self.files_change_mark(change))
                    .into_any_element()
            }
        })
    }

    /// `row` as a file's: its icon, its name's color for its change, the
    /// selection, and the click that opens it.
    fn files_file_row(
        &self,
        row: Stateful<Div>,
        path: &str,
        change: Option<&Change>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = &self.theme;
        let selected = self.files.selected.as_deref() == Some(path);
        let color = match change {
            Some(change) if change.new => theme.ink(theme.palette[2]),
            Some(_) => theme.ink(theme.palette[3]),
            None => theme.foreground,
        };
        let opened = path.to_owned();
        row.cursor_pointer()
            .text_color(rgb(color))
            .when(selected, |row| row.bg(rgb(theme.active)))
            .hover(|style| style.bg(rgb(theme.active)))
            .child(
                svg()
                    .path("icons/file.svg")
                    .size(px(12.))
                    .flex_none()
                    .text_color(rgb(theme.muted)),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                let opening = if event.modifiers().secondary() {
                    Opening::Editor
                } else {
                    Opening::Tab
                };
                this.open_files_entry(&opened, opening, window, cx);
            }))
    }

    /// A changed file's lines added and removed, or that it is new.
    fn files_change_mark(&self, change: Option<&Change>) -> Div {
        let theme = &self.theme;
        let mark = div()
            .flex_none()
            .text_size(px(self.config.sidebar.size - 2.));
        match change {
            Some(change) if change.new => {
                mark.text_color(rgb(theme.ink(theme.palette[2]))).child("U")
            }
            Some(Change {
                counts: Some((added, removed)),
                ..
            }) => mark
                .flex()
                .gap_1()
                .child(
                    div()
                        .text_color(rgb(theme.ink(theme.palette[2])))
                        .child(format!("+{added}")),
                )
                .child(
                    div()
                        .text_color(rgb(theme.ink(theme.palette[1])))
                        .child(format!("−{removed}")),
                ),
            Some(_) => mark.text_color(rgb(theme.ink(theme.palette[3]))).child("M"),
            None => mark,
        }
    }

    /// Opens `path`, relative to the shown checkout, as `opening` says.
    pub(crate) fn open_files_entry(
        &mut self,
        path: &str,
        opening: Opening,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(root) = self.files.root() else {
            return;
        };
        let target = EditorTarget {
            path: root.join(path),
            line: None,
        };
        self.files.selected = Some(path.to_owned());
        match opening {
            Opening::Editor => self.open_in_editor(&target, None, cx),
            Opening::Tab if crate::browser::renders_markdown(&target.path) => {
                self.open_markdown(&target.path, window, cx)
            }
            Opening::Tab | Opening::Source => self.open_code_view(&target, window, cx),
        }
        // Counted now, not at the next tick: until then the folder's Files
        // would leave the new tab out, and its group would forget it.
        self.sync_file_tabs(cx);
        cx.notify();
    }
}
