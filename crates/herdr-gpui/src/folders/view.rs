//! The Folders page as drawn: the folder list on the left (`list`), and the
//! selected folder's part across the rest, its Home (`home`) or its
//! Changes. The marks, branch lines, and badges both sides share are here.

use super::{
    Part,
    model::{self, Card},
};
use crate::{
    HerdrWindow,
    config::{corners, mix},
    devices_overview::Look,
    fonts::StyledFont,
};
use gpui::{prelude::*, *};
use herdr_client::protocol::AgentStatus;

impl HerdrWindow {
    /// The page across the window's body.
    pub(crate) fn render_folders(&self, cx: &mut Context<Self>) -> AnyElement {
        let look = self.overview_look(cx);
        let theme = look.theme;
        let cards = self.folder_cards(cx);
        let listed = self.listed_folders(&cards);
        let selected = self.selected_folder(&listed);
        let body = match selected {
            None if cards.is_empty() => empty(&look).into_any_element(),
            None => div()
                .debug_selector(|| "folders-settled".into())
                .p_5()
                .text_color(rgb(theme.muted))
                .child("Nothing needs you in any folder.")
                .into_any_element(),
            Some(card) => match self.folders.part {
                Part::Changes => self.render_folder_changes(&look, card, cx),
                // The other parts are a column beside the terminals, and
                // never drawn as this page.
                Part::Home | Part::Terminals | Part::Files => {
                    self.render_folder_home(&look, card, cx).into_any_element()
                }
            },
        };
        div()
            .id("folders")
            .debug_selector(|| "folders".into())
            .flex()
            .flex_1()
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(rgb(theme.background))
            .text_color(rgb(theme.foreground))
            .text_font(look.ui)
            .text_size(px(look.ui.size))
            .child(self.render_folder_list(&look, &cards, &listed, selected, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(body),
            )
            .into_any_element()
    }

    /// The selected folder's checkout with the review beside it, as the
    /// Changes section draws them, for that checkout alone.
    fn render_folder_changes(
        &self,
        look: &Look,
        card: &Card,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = look.theme;
        let shown = card.checkout.as_ref().and_then(|path| {
            self.changes
                .repos()
                .iter()
                .flat_map(|repo| &repo.checkouts)
                .find(|shown| shown.checkout.path == *path)
        });
        let list = match shown {
            Some(shown) => div()
                .rounded(px(corners::PANEL))
                .bg(rgb(theme.surface))
                .child(self.changes_checkout(0, 0, shown, cx))
                .into_any_element(),
            None => div()
                .text_color(rgb(theme.muted))
                .child(if card.host.is_some() {
                    "Uncommitted files are read on this computer's checkouts only, so far."
                } else {
                    "This folder is not in a Git checkout."
                })
                .into_any_element(),
        };
        div()
            .debug_selector(|| "folder-changes".into())
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(
                folder_header(look, card)
                    .flex_none()
                    .px_5()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(theme.active)),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        self.resizable_panel(
                            div()
                                .id("folder-changes-panel")
                                .debug_selector(|| "folder-changes-panel".into())
                                .flex_none()
                                .min_h_0()
                                .child(
                                    div()
                                        .id("folder-changes-list")
                                        .size_full()
                                        .overflow_y_scroll()
                                        .p_3()
                                        .child(list),
                                ),
                            "folder-changes-resize",
                            crate::panel_resize::PanelDrag::ChangesList,
                            None,
                            cx,
                        ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .flex()
                            .border_l_1()
                            .border_color(rgb(theme.active))
                            .when(shown.is_some(), |pane| {
                                pane.child(self.render_changes_review(cx))
                            }),
                    ),
            )
            .into_any_element()
    }
}

/// The color that tells a folder apart: its mark and its row's edge bar.
pub(super) fn folder_color(look: &Look, name: &str) -> u32 {
    let theme = look.theme;
    theme.ink(theme.palette[model::hue(name)])
}

/// A folder's mark: its monogram on a wash of its own hue.
pub(super) fn mark(look: &Look, name: &str, side: f32) -> Div {
    let theme = look.theme;
    let color = folder_color(look, name);
    div()
        .flex_none()
        .size(px(side))
        .rounded(px(corners::CONTROL))
        .bg(rgb(mix(theme.background, color, 22)))
        .border_1()
        .border_color(rgb(mix(theme.background, color, 45)))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(color))
        .text_size(px(side * 0.38))
        .font_weight(FontWeight::BOLD)
        .child(model::monogram(name))
}

/// The branch in the terminal face, then ahead and behind when nonzero.
pub(super) fn branch_line(look: &Look, card: &Card) -> Div {
    let theme = look.theme;
    div()
        .flex()
        .items_center()
        .gap_1p5()
        .min_w_0()
        .text_size(look.small())
        .when_some(card.branch.clone(), |line, branch| {
            line.child(
                svg()
                    .flex_none()
                    .size(px(11.))
                    .path("icons/git-branch.svg")
                    .text_color(rgb(theme.muted)),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_font(look.mono)
                    .text_size(px(look.mono.size - 1.))
                    .text_color(rgb(theme.subtext()))
                    .child(branch),
            )
        })
        .when(card.ahead > 0, |line| {
            line.child(
                div()
                    .flex_none()
                    .text_color(rgb(look.status(AgentStatus::Working)))
                    .child(format!("↑{}", card.ahead)),
            )
        })
        .when(card.behind > 0, |line| {
            line.child(
                div()
                    .flex_none()
                    .text_color(rgb(theme.subtext()))
                    .child(format!("↓{}", card.behind)),
            )
        })
}

/// A count on a solid fill, as the bar's bubbles draw one.
pub(super) fn bubble(look: &Look, fill: u32, count: usize) -> Div {
    let theme = look.theme;
    div()
        .flex_none()
        .px(px(5.))
        .min_w(px(16.))
        .h(px(16.))
        .rounded_full()
        .bg(rgb(fill))
        .text_color(rgb(theme.text_on(fill)))
        .text_size(px(10.))
        .font_weight(FontWeight::BOLD)
        .flex()
        .items_center()
        .justify_center()
        .child(count.to_string())
}

/// The host a folder is on, when it is not this computer.
pub(super) fn host_chip(look: &Look, host: String) -> Div {
    let theme = look.theme;
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap_1()
        .px_1p5()
        .rounded(px(corners::SMALL))
        .bg(rgb(theme.active))
        .text_size(look.small())
        .text_color(rgb(theme.subtext()))
        .child(
            svg()
                .size(px(11.))
                .path("icons/computer.svg")
                .text_color(rgb(theme.muted)),
        )
        .child(host)
}

/// The selected folder's title: mark, name, branch, and where it is.
pub(super) fn folder_header(look: &Look, card: &Card) -> Div {
    let theme = look.theme;
    let folder = if card.host.is_some() {
        card.folder.clone()
    } else {
        crate::folder::home_relative(&card.folder)
    };
    div()
        .flex()
        .items_center()
        .gap_3()
        .min_w_0()
        .child(mark(look, &card.name, 34.))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .truncate()
                        .text_size(px(look.ui.size + 3.))
                        .font_weight(FontWeight::BOLD)
                        .child(card.name.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .min_w_0()
                        .child(branch_line(look, card))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(look.small())
                                .text_color(rgb(theme.muted))
                                .child(folder),
                        ),
                ),
        )
        .when_some(card.host.clone(), |header, host| {
            header.child(host_chip(look, host))
        })
}

/// What the page is for, while no workspace is open.
fn empty(look: &Look) -> Div {
    let theme = look.theme;
    div()
        .debug_selector(|| "folders-empty".into())
        .size_full()
        .min_h(px(320.))
        .px_6()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .child(
            svg()
                .path("icons/folder.svg")
                .size(px(22.))
                .text_color(rgb(theme.muted)),
        )
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child("No folders open"),
        )
        .child(
            div()
                .max_w(px(460.))
                .text_center()
                .text_color(rgb(theme.muted))
                .child(
                    "Folders lists every workspace open on every host this window \
                     connects to, with what waits on you first. Pick one to see its \
                     uncommitted files, agents, commits, and ports in one place.",
                ),
        )
}
