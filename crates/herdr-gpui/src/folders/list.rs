//! The folder list down the page's left side: a row per folder, its name,
//! branch, host, and badges for uncommitted files and its agent, and
//! under the selected one the parts it holds, each shown beside the list.
//! The selected folder's own row is its Home.

use super::{
    Part,
    drag::Rows,
    model::Card,
    view::{bubble, folder_color, host_chip},
};
use crate::{
    HerdrWindow,
    config::{corners, mix},
    devices_overview::Look,
    panel_resize::PanelDrag,
};
use gpui::{prelude::*, *};
use herdr_client::protocol::AgentStatus;

/// The space between folder rows, which a drag's preview counts in each
/// row's height.
const ROW_GAP: f32 = 2.;

impl HerdrWindow {
    pub(super) fn render_folder_list(
        &self,
        look: &Look,
        cards: &[Card],
        selected: Option<&Card>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = look.theme;
        let summary = super::model::Summary::of(cards);
        let now = crate::motion::now();
        // A lifted drag's rows and where each paints, from the heights the
        // last frame laid out: shifting moves rows, never resizes them.
        let drag = self.folders.drag.as_ref();
        let prepared = drag.zip(self.folder_drag_rows(cards)).map(|(drag, rows)| {
            let heights: Vec<f32> = rows
                .items
                .iter()
                .map(|&item| {
                    self.folders
                        .list_scroll
                        .bounds_for_item(item)
                        .map_or(0., |bounds| f32::from(bounds.size.height) + ROW_GAP)
                })
                .collect();
            let (shifts, sliding) = drag.shifts(&rows, &heights, now);
            (rows, shifts, sliding)
        });
        let shift_of = |item: usize| {
            prepared.as_ref().and_then(|(rows, shifts, _)| {
                let unit = rows.items.iter().position(|&row| row == item)?;
                shifts.get(unit).copied()
            })
        };
        let list = div()
            .id("folders-list")
            .debug_selector(|| "folders-list".into())
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .bg(self.fill().panel(theme.sidebar_background()))
            .border_r_1()
            .border_color(rgb(theme.active))
            .child(
                div()
                    .debug_selector(|| "folders-summary".into())
                    .flex_none()
                    .px_3()
                    .pt_3()
                    .pb_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Folders"),
                    )
                    .when(summary.waiting > 0, |head| {
                        head.child(bubble(
                            look,
                            look.status(AgentStatus::Blocked),
                            summary.waiting,
                        ))
                    })
                    .child(self.folder_new(look, cx)),
            )
            .child(
                div()
                    .id("folders-rows")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.folders.list_scroll)
                    .px_2()
                    .pb_2()
                    .flex()
                    .flex_col()
                    .gap(px(ROW_GAP))
                    .children(cards.iter().enumerate().map(|(item, card)| {
                        let open = selected.is_some_and(|selected| {
                            selected.endpoint == card.endpoint
                                && selected.workspace == card.workspace
                        });
                        let carried = drag
                            .is_some_and(|drag| drag.carries(card.endpoint_index, &card.workspace));
                        let row = self.folder_row(look, card, open, carried, cx);
                        let unit = div()
                            .relative()
                            .flex()
                            .flex_col()
                            .child(row)
                            .when(open, |unit| unit.child(self.folder_parts(look, card, cx)))
                            .when_some(
                                shift_of(item).filter(|shift| *shift != 0.),
                                |unit, shift| unit.top(px(shift)),
                            );
                        if carried {
                            // Painted last so it floats over the rows it
                            // passes, while its slot keeps theirs stable.
                            deferred(unit).with_priority(1).into_any_element()
                        } else {
                            unit.into_any_element()
                        }
                    })),
            )
            .when(drag.is_some(), |list| {
                list.child(self.folder_drag_capture(prepared, cx))
            });
        self.resizable_panel(
            list,
            "folders-list-resize",
            PanelDrag::FoldersList,
            None,
            cx,
        )
    }

    /// The + that opens a project from the palette, recent ones first.
    fn folder_new(&self, look: &Look, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = look.theme;
        let (foreground, surface) = (theme.foreground, theme.surface);
        div()
            .id("folders-new")
            .debug_selector(|| "folders-new".into())
            .flex_none()
            .p_0p5()
            .rounded(px(corners::SMALL))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(theme.active)))
            .tooltip(move |_, cx| {
                cx.new(|_| crate::menu::SettingsHint {
                    text: "Open project".into(),
                    foreground,
                    surface,
                })
                .into()
            })
            .on_click(cx.listener(|this, _, window, cx| {
                this.open_project_from_folders(window, cx);
            }))
            .child(
                svg()
                    .size(px(13.))
                    .path("icons/plus.svg")
                    .text_color(rgb(theme.muted)),
            )
    }

    /// Follows the pointer anywhere in the window while a folder drag is
    /// armed, so the drag continues off the list and its release is never a
    /// click on what lies under it.
    fn folder_drag_capture(
        &self,
        prepared: Option<(Rows, Vec<f32>, bool)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity();
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                let (rows, sliding) = match prepared {
                    Some((rows, shifts, sliding)) => (Some((rows, shifts)), sliding),
                    None => (None, false),
                };
                if sliding {
                    window.request_animation_frame();
                }
                let moving = view.clone();
                let moved = rows.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    moving.update(cx, |this, cx| {
                        let held = event.pressed_button == Some(MouseButton::Left);
                        if this.move_folder_drag(event.position, held, moved.as_ref(), cx) {
                            cx.stop_propagation();
                        }
                    });
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                        return;
                    }
                    view.update(cx, |this, cx| {
                        // A lifted row's release is its drop, not a click.
                        let rows = rows.as_ref().map(|(rows, _)| rows);
                        if this.release_folder_drag(rows, cx) {
                            cx.stop_propagation();
                        }
                    });
                });
            },
        )
        .absolute()
        .size_full()
    }

    fn folder_row(
        &self,
        look: &Look,
        card: &Card,
        open: bool,
        carried: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = look.theme;
        let selector = format!("folder-{}-{}", card.endpoint, card.workspace);
        let bar = format!("folder-bar-{}-{}", card.endpoint, card.workspace);
        let clicked = card.clone();
        let pressed = card.clone();
        let menu = card.clone();
        let waiting = card.waiting();
        div()
            .id(SharedString::from(selector.clone()))
            .debug_selector(move || selector.clone())
            .relative()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1p5()
            .rounded(px(corners::CONTROL))
            .cursor_pointer()
            // The folder's own color down its left edge, as its Home's mark,
            // so folders tell apart at a glance without taking width.
            .child(
                div()
                    .debug_selector(move || bar.clone())
                    .absolute()
                    .left_0()
                    .top(px(8.))
                    .bottom(px(8.))
                    .w(px(3.))
                    .rounded_full()
                    .bg(rgb(folder_color(look, card.title()))),
            )
            .when(open, |row| {
                row.bg(rgb(if self.folders.part == Part::Home {
                    theme.primary_wash()
                } else {
                    theme.active
                }))
            })
            .when(!open && !carried, |row| {
                row.hover(|style| style.bg(rgb(mix(theme.surface, theme.foreground, 4))))
            })
            .when(carried, |row| {
                row.cursor_grabbing().bg(rgb(theme.active)).shadow_lg()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    if event.click_count == 1 {
                        this.press_folder(&pressed, event.position, cx);
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.open_folder_menu(&menu, event.position, window, cx);
                }),
            )
            // The open folder's row is its Home; another folder opens at
            // the part it was left on.
            .on_click(cx.listener(move |this, _, window, cx| {
                let part = open.then_some(Part::Home);
                this.select_folder(&clicked, part, window, cx);
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(super::view::title_line(look, card, look.ui.size))
                    .child(super::view::path_line(look, card))
                    .when(card.shows_branch(), |column| {
                        column.child(super::view::branch_line(look, card))
                    })
                    // What runs here, under the branch, so it is seen
                    // without opening the folder's Home.
                    .when_some(self.folder_ports(look, card, cx), |column, ports| {
                        let selector = format!("folder-ports-{}", card.workspace);
                        column.child(
                            div()
                                .debug_selector(move || selector.clone())
                                .text_size(look.small())
                                .child(ports),
                        )
                    }),
            )
            .when_some(card.host.clone(), |row, host| {
                row.child(host_chip(look, host))
            })
            .when_some(card.dirty.filter(|dirty| dirty.files > 0), |row, dirty| {
                row.child(
                    bubble(look, look.status(AgentStatus::Blocked), dirty.files)
                        .when(!waiting, |bubble| {
                            bubble.bg(rgb(theme.ink(theme.palette[3])))
                        }),
                )
            })
            .when_some(card.agent.as_ref(), |row, agent| {
                row.child(look.dot(agent.state.status()))
            })
    }

    /// The selected folder's parts, hung under its row.
    fn folder_parts(&self, look: &Look, card: &Card, cx: &mut Context<Self>) -> Div {
        let theme = look.theme;
        let files = card
            .dirty
            .map(|dirty| dirty.files)
            .filter(|files| *files > 0);
        let ahead = card.ahead;
        let parts: [(&str, &str, &str, Option<usize>, Part); 3] = [
            (
                "terminals",
                "Terminals",
                "icons/terminal.svg",
                Some(card.tabs).filter(|tabs| *tabs > 0),
                Part::Terminals,
            ),
            (
                "changes",
                "Changes",
                "icons/git-branch.svg",
                files,
                Part::Changes,
            ),
            ("files", "Files", "icons/file.svg", None, Part::Files),
        ];
        div()
            .flex()
            .flex_col()
            .ml(px(10.))
            .pl_2()
            .my_1()
            .border_l_1()
            .border_color(rgb(theme.active))
            .children(parts.into_iter().map(|(key, label, icon, count, part)| {
                let picked = part == self.folders.part;
                let selector = format!("folder-part-{key}");
                let clicked = card.clone();
                div()
                    .id(SharedString::from(selector.clone()))
                    .debug_selector(move || selector.clone())
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded(px(corners::SMALL))
                    .cursor_pointer()
                    .when(picked, |row| {
                        row.bg(rgb(theme.primary_wash()))
                            .font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!picked, |row| {
                        row.text_color(rgb(theme.subtext()))
                            .hover(|style| style.bg(rgb(theme.active)))
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_folder(&clicked, Some(part), window, cx);
                    }))
                    .child(
                        svg()
                            .flex_none()
                            .size(px(13.))
                            .path(icon)
                            .text_color(rgb(theme.muted)),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(label))
                    .when_some(count, |row, count| {
                        row.child(
                            div()
                                .text_size(look.small())
                                .text_color(rgb(theme.muted))
                                .child(count.to_string()),
                        )
                    })
                    // Commits not pushed yet, as the branch line marks them.
                    .when(part == Part::Changes && ahead > 0, |row| {
                        row.child(
                            div()
                                .debug_selector(|| "folder-part-changes-ahead".into())
                                .text_size(look.small())
                                .text_color(rgb(look.status(AgentStatus::Working)))
                                .child(format!("\u{2191}{ahead}")),
                        )
                    })
            }))
    }
}
