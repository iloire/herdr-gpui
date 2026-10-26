//! The folder list down the page's left side: a row per folder, its mark,
//! name, branch, host, and badges for uncommitted files and its agent, and
//! under the selected one the parts it holds, each shown beside the list.

use super::{
    Part,
    model::{Card, Rank},
    view::{bubble, host_chip, mark},
};
use crate::{
    HerdrWindow,
    config::{corners, mix},
    devices_overview::Look,
};
use gpui::{prelude::*, *};
use herdr_client::protocol::AgentStatus;

pub(super) const LIST_WIDTH: f32 = 290.;

impl HerdrWindow {
    pub(super) fn render_folder_list(
        &self,
        look: &Look,
        cards: &[Card],
        listed: &[&Card],
        selected: Option<&Card>,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = look.theme;
        let summary = super::model::Summary::of(cards);
        div()
            .debug_selector(|| "folders-list".into())
            .flex_none()
            .w(px(LIST_WIDTH))
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
                    .child(self.folder_filter(look, cx)),
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
                    .gap_0p5()
                    .children(listed.iter().map(|card| {
                        let open = selected.is_some_and(|selected| {
                            selected.endpoint == card.endpoint
                                && selected.workspace == card.workspace
                        });
                        let row = self.folder_row(look, card, open, cx);
                        if open {
                            div()
                                .flex()
                                .flex_col()
                                .child(row)
                                .child(self.folder_parts(look, card, cx))
                        } else {
                            div().child(row)
                        }
                    }))
                    .when(listed.is_empty() && !cards.is_empty(), |rows| {
                        rows.child(
                            div()
                                .p_2()
                                .text_size(look.small())
                                .text_color(rgb(theme.muted))
                                .child("Nothing needs you."),
                        )
                    }),
            )
    }

    /// Needs me or All.
    fn folder_filter(&self, look: &Look, cx: &mut Context<Self>) -> Div {
        let theme = look.theme;
        let option = |id: &'static str, label: &'static str, all: bool, cx: &mut Context<Self>| {
            let on = self.folders.all == all;
            div()
                .id(id)
                .debug_selector(move || id.into())
                .px_1p5()
                .rounded(px(corners::SMALL))
                .cursor_pointer()
                .text_size(look.small())
                .when(on, |pill| pill.bg(rgb(theme.active)))
                .when(!on, |pill| pill.text_color(rgb(theme.muted)))
                .on_click(cx.listener(move |this, _, _, cx| this.show_all_folders(all, cx)))
                .child(label)
        };
        div()
            .flex()
            .flex_none()
            .gap_0p5()
            .child(option("folders-needs-me", "Needs me", false, cx))
            .child(option("folders-all", "All", true, cx))
    }

    fn folder_row(
        &self,
        look: &Look,
        card: &Card,
        open: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = look.theme;
        let selector = format!("folder-{}-{}", card.endpoint, card.workspace);
        let clicked = card.clone();
        let needs = card.rank() == Rank::NeedsYou;
        div()
            .id(SharedString::from(selector.clone()))
            .debug_selector(move || selector.clone())
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1p5()
            .rounded(px(corners::CONTROL))
            .cursor_pointer()
            .when(open, |row| row.bg(rgb(theme.active)))
            .when(!open, |row| {
                row.hover(|style| style.bg(rgb(mix(theme.surface, theme.foreground, 4))))
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_folder(&clicked, None, window, cx);
            }))
            .child(mark(look, &card.name, 28.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(card.name.clone()),
                    )
                    .child(super::view::branch_line(look, card)),
            )
            .when_some(card.host.clone(), |row, host| {
                row.child(host_chip(look, host))
            })
            .when_some(card.dirty.filter(|dirty| dirty.files > 0), |row, dirty| {
                row.child(
                    bubble(look, look.status(AgentStatus::Blocked), dirty.files)
                        .when(!needs, |bubble| bubble.bg(rgb(theme.ink(theme.palette[3])))),
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
        let parts: [(&str, &str, &str, Option<usize>, Part); 4] = [
            ("home", "Home", "icons/pulse.svg", None, Part::Home),
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
            .ml(px(22.))
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
            }))
    }
}
