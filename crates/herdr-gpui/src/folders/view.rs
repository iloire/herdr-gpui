//! The Folders page as drawn: a summary line with the Needs me and All
//! filters, then the cards, wrapping to the window's width. With nothing
//! open, the page says what it is for.

use super::model::{self, Card, Summary};
use crate::{
    HerdrWindow,
    config::{corners, mix},
    devices_overview::Look,
    fonts::StyledFont,
    icons::AgentIcon,
};
use gpui::{prelude::*, *};

/// The least a card takes before the row wraps.
const CARD_MIN: f32 = 300.;
const MARK: f32 = 30.;

impl HerdrWindow {
    /// The page across the window's body.
    pub(crate) fn render_folders(&self, cx: &mut Context<Self>) -> AnyElement {
        let look = self.overview_look(cx);
        let theme = look.theme;
        let cards = self.folder_cards(cx);
        let summary = Summary::of(&cards);
        let shown: Vec<&Card> = cards
            .iter()
            .filter(|card| self.folders.all || card.needs_me())
            .collect();
        let body = if cards.is_empty() {
            empty(&look).into_any_element()
        } else if shown.is_empty() {
            div()
                .debug_selector(|| "folders-settled".into())
                .p_5()
                .text_color(rgb(theme.muted))
                .child("Nothing needs you in any folder.")
                .into_any_element()
        } else {
            div()
                .debug_selector(|| "folders-cards".into())
                .p_4()
                .flex()
                .flex_wrap()
                .gap_3()
                .children(
                    shown
                        .into_iter()
                        .map(|card| self.render_folder_card(&look, card, cx)),
                )
                .into_any_element()
        };
        div()
            .id("folders")
            .debug_selector(|| "folders".into())
            .flex()
            .flex_col()
            .flex_1()
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(rgb(theme.background))
            .text_color(rgb(theme.foreground))
            .text_font(look.ui)
            .text_size(px(look.ui.size))
            .child(self.folders_summary(&look, summary, cards.len(), cx))
            .child(
                div()
                    .id("folders-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.folders.scroll)
                    .child(body),
            )
            .into_any_element()
    }

    fn folders_summary(
        &self,
        look: &Look,
        summary: Summary,
        total: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = look.theme;
        let mut parts = Vec::new();
        if summary.waiting > 0 {
            parts.push(format!("{} waiting for you", summary.waiting));
        }
        if summary.dirty > 0 {
            parts.push(format!("{} uncommitted", summary.dirty));
        }
        if summary.unpushed > 0 {
            parts.push(format!("{} to push", summary.unpushed));
        }
        if summary.hosts > 1 {
            parts.push(format!("{} hosts", summary.hosts));
        }
        let filter = |id: &'static str, label: &'static str, all: bool, cx: &mut Context<Self>| {
            let on = self.folders.all == all;
            div()
                .id(id)
                .debug_selector(move || id.into())
                .px_2p5()
                .py_1()
                .rounded(px(corners::CONTROL))
                .cursor_pointer()
                .when(on, |pill| {
                    pill.bg(rgb(theme.active)).font_weight(FontWeight::SEMIBOLD)
                })
                .when(!on, |pill| {
                    pill.text_color(rgb(theme.subtext()))
                        .hover(|style| style.bg(rgb(theme.surface)))
                })
                .on_click(cx.listener(move |this, _, _, cx| this.show_all_folders(all, cx)))
                .child(label)
        };
        div()
            .debug_selector(|| "folders-summary".into())
            .flex_none()
            .px_5()
            .py_3()
            .flex()
            .items_center()
            .gap_3()
            .border_b_1()
            .border_color(rgb(theme.active))
            .child(
                div()
                    .text_size(px(look.ui.size + 3.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Folders"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(rgb(theme.subtext()))
                    .child(if parts.is_empty() {
                        format!("{total} open, all quiet")
                    } else {
                        parts.join(" · ")
                    }),
            )
            .child(filter("folders-needs-me", "Needs me", false, cx))
            .child(filter("folders-all", "All", true, cx))
    }

    fn render_folder_card(&self, look: &Look, card: &Card, cx: &mut Context<Self>) -> AnyElement {
        let theme = look.theme;
        let selector = format!("folder-{}-{}", card.endpoint, card.workspace);
        let needs = card.rank() == model::Rank::NeedsYou;
        let border = if needs {
            look.status(herdr_client::protocol::AgentStatus::Blocked)
        } else {
            mix(theme.surface, theme.foreground, 8)
        };
        let (index, workspace) = (card.endpoint_index, card.workspace.clone());
        let ports = self
            .endpoints
            .get(card.endpoint_index)
            .and_then(|endpoint| crate::listening_ports::Daemon::of(&endpoint.connection.target))
            .filter(|_| self.config.show_listening_ports)
            .and_then(|daemon| self.listening_ports.get(&daemon, &card.workspace))
            .map(|listed| {
                crate::listening_ports::chips(
                    listed,
                    (&card.endpoint, &card.workspace),
                    &self.tunnels,
                    theme,
                    look.ui.size - 1.,
                    cx,
                )
            });
        div()
            .id(SharedString::from(selector.clone()))
            .debug_selector(move || selector.clone())
            .flex()
            .flex_col()
            .gap_2()
            .flex_1()
            .min_w(px(CARD_MIN))
            .p_3()
            .rounded(px(corners::PANEL))
            .bg(rgb(theme.surface))
            .border_1()
            .border_color(rgb(border))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(mix(theme.surface, theme.foreground, 4))))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_folder(index, &workspace, window, cx);
            }))
            .child(title(look, card))
            .child(git_line(look, card, ports))
            .child(agent_line(look, card))
            .into_any_element()
    }
}

/// The folder's mark: its monogram on a wash of its own hue.
fn mark(look: &Look, name: &str) -> Div {
    let theme = look.theme;
    let color = theme.ink(theme.palette[model::hue(name)]);
    div()
        .flex_none()
        .size(px(MARK))
        .rounded(px(corners::CONTROL))
        .bg(rgb(mix(theme.background, color, 22)))
        .border_1()
        .border_color(rgb(mix(theme.background, color, 45)))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(color))
        .text_size(px(MARK * 0.36))
        .font_weight(FontWeight::BOLD)
        .child(model::monogram(name))
}

/// Mark, name, branch with ahead and behind, and the host when remote.
fn title(look: &Look, card: &Card) -> Div {
    let theme = look.theme;
    let sync = div()
        .flex()
        .flex_none()
        .gap_1()
        .text_size(look.small())
        .when(card.ahead > 0, |sync| {
            sync.child(
                div()
                    .text_color(rgb(
                        look.status(herdr_client::protocol::AgentStatus::Working)
                    ))
                    .child(format!("↑{}", card.ahead)),
            )
        })
        .when(card.behind > 0, |sync| {
            sync.child(
                div()
                    .text_color(rgb(theme.subtext()))
                    .child(format!("↓{}", card.behind)),
            )
        });
    div()
        .flex()
        .items_center()
        .gap_2()
        .min_w_0()
        .child(mark(look, &card.name))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .truncate()
                        .font_weight(FontWeight::BOLD)
                        .child(card.name.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .min_w_0()
                        .when_some(card.branch.clone(), |line, branch| {
                            line.child(
                                svg()
                                    .flex_none()
                                    .size(px(12.))
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
                        .child(sync),
                ),
        )
        .when_some(card.host.clone(), |title, host| {
            title.child(
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
                    .child(host),
            )
        })
}

/// Uncommitted files and their lines, or clean, then the ports.
fn git_line(look: &Look, card: &Card, ports: Option<Div>) -> Div {
    let theme = look.theme;
    let line = div()
        .flex()
        .items_center()
        .gap_2()
        .min_h(px(18.))
        .text_size(look.small());
    let line = match card.dirty {
        Some(dirty) if dirty.files > 0 => {
            let warn = look.status(herdr_client::protocol::AgentStatus::Blocked);
            line.child(
                div()
                    .flex_none()
                    .px(px(5.))
                    .min_w(px(16.))
                    .h(px(16.))
                    .rounded_full()
                    .bg(rgb(warn))
                    .text_color(rgb(theme.text_on(warn)))
                    .text_size(px(10.))
                    .font_weight(FontWeight::BOLD)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(dirty.files.to_string()),
            )
            .child(div().text_color(rgb(theme.subtext())).child("uncommitted"))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .text_font(look.mono)
                    .child(
                        div()
                            .text_color(rgb(theme.ink(theme.palette[2])))
                            .child(format!("+{}", dirty.added)),
                    )
                    .child(
                        div()
                            .text_color(rgb(theme.ink(theme.palette[1])))
                            .child(format!("−{}", dirty.removed)),
                    ),
            )
        }
        Some(_) => line.child(div().text_color(rgb(theme.muted)).child("clean")),
        None => line,
    };
    line.child(div().flex_1()).children(ports)
}

/// The most pressing agent, or a quiet line saying there is none.
fn agent_line(look: &Look, card: &Card) -> Div {
    let theme = look.theme;
    let Some(agent) = &card.agent else {
        return div()
            .p_2()
            .rounded(px(corners::CONTROL))
            .border_1()
            .border_dashed()
            .border_color(rgb(theme.active))
            .text_color(rgb(theme.muted))
            .child("No agent running");
    };
    let color = look.status(agent.state.status());
    let more = agent.count - 1;
    div()
        .flex()
        .items_center()
        .gap_2()
        .min_w_0()
        .p_2()
        .rounded(px(corners::CONTROL))
        .bg(rgb(mix(theme.surface, color, 12)))
        .child(crate::icons::icon(
            AgentIcon::from_identity(agent.identity.as_deref()).path(),
            14.,
            theme.foreground,
        ))
        .child(look.dot(agent.state.status()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(agent.title.clone()),
        )
        .when(more > 0, |line| {
            line.child(
                div()
                    .flex_none()
                    .text_size(look.small())
                    .text_color(rgb(theme.muted))
                    .child(format!("+{more}")),
            )
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
                    "Folders shows every workspace open on every host this window \
                     connects to, as a card: its branch, uncommitted files, agents, and \
                     ports, with what waits on you first. Click a card to open it.",
                ),
        )
}
