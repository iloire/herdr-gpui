//! A folder's Home: its title with buttons to its terminals and files,
//! then cards for what it holds. Uncommitted files and recent commits open
//! its Changes; an agent opens its pane in its Terminals; a port opens in
//! a browser tab.

use super::{
    Part,
    model::{self, Card},
    view::folder_header,
};
use crate::{
    HerdrWindow,
    config::{corners, mix},
    devices_overview::Look,
    fonts::StyledFont,
    icons::AgentIcon,
};
use gpui::{prelude::*, *};
use herdr_vcs::changes::{Checkout, Kind};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Files and commits a card lists before it says how many more.
const LISTED: usize = 8;

impl HerdrWindow {
    pub(super) fn render_folder_home(
        &self,
        look: &Look,
        card: &Card,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let checkout = card.checkout.as_ref().and_then(|path| {
            self.changes
                .repos()
                .iter()
                .flat_map(|repo| &repo.checkouts)
                .map(|shown| &shown.checkout)
                .find(|checkout| checkout.path == *path)
        });
        let terminals = card.clone();
        let files = card.clone();
        div()
            .id("folder-home")
            .debug_selector(|| "folder-home".into())
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.folders.body_scroll)
            .p_5()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                folder_header(look, card)
                    .child(
                        button(look, "folder-open-terminals", "Terminals", true).on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.select_folder(&terminals, Some(Part::Terminals), window, cx);
                            }),
                        ),
                    )
                    .child(button(look, "folder-open-files", "Files", false).on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.select_folder(&files, Some(Part::Files), window, cx);
                        }),
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .min_w_0()
                    .child(self.home_uncommitted(look, card, checkout, cx))
                    .child(self.home_agents(look, card, cx)),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .min_w_0()
                    .child(self.home_commits(look, card, checkout, cx))
                    .child(self.home_ports(look, card, cx)),
            )
    }

    fn home_uncommitted(
        &self,
        look: &Look,
        card: &Card,
        checkout: Option<&Checkout>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = look.theme;
        let count = card.dirty.filter(|dirty| dirty.files > 0).map(|dirty| {
            format!(
                "{} {} · +{} −{}",
                dirty.files,
                if dirty.files == 1 { "file" } else { "files" },
                dirty.added,
                dirty.removed
            )
        });
        let body: AnyElement = match checkout {
            Some(checkout) if checkout.dirty() => div()
                .flex()
                .flex_col()
                .gap_1()
                .children(checkout.files.iter().take(LISTED).map(|file| {
                    let (letter, color) = match file.kind {
                        Kind::Untracked => ("?", theme.ink(theme.palette[2])),
                        Kind::Added => ("A", theme.ink(theme.palette[2])),
                        Kind::Deleted => ("D", theme.ink(theme.palette[1])),
                        Kind::Conflicted => ("U", theme.ink(theme.palette[1])),
                        Kind::Renamed => ("R", theme.ink(theme.palette[4])),
                        Kind::Modified => ("M", theme.ink(theme.palette[3])),
                    };
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .min_w_0()
                        .text_font(look.mono)
                        .text_size(px(look.mono.size - 1.))
                        .child(
                            div()
                                .flex_none()
                                .w(px(12.))
                                .text_color(rgb(color))
                                .child(letter),
                        )
                        .child(div().flex_1().min_w_0().truncate().child(file.path.clone()))
                        .when_some(
                            file.additions.zip(file.deletions),
                            |row, (added, removed)| {
                                row.child(
                                    div()
                                        .flex_none()
                                        .flex()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_color(rgb(theme.ink(theme.palette[2])))
                                                .child(format!("+{added}")),
                                        )
                                        .when(removed > 0, |lines| {
                                            lines.child(
                                                div()
                                                    .text_color(rgb(theme.ink(theme.palette[1])))
                                                    .child(format!("−{removed}")),
                                            )
                                        }),
                                )
                            },
                        )
                }))
                .children(more(look, checkout.total_files))
                .into_any_element(),
            Some(_) => quiet(look, "Nothing uncommitted").into_any_element(),
            None if card.host.is_some() => quiet(
                look,
                "Uncommitted files are read on this computer's checkouts only, so far.",
            )
            .into_any_element(),
            None => quiet(look, "Not a Git checkout").into_any_element(),
        };
        let clicked = card.clone();
        panel(look, "folder-home-uncommitted", "Uncommitted", count)
            .when(checkout.is_some(), |panel| {
                panel
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(mix(theme.surface, theme.foreground, 4))))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_folder(&clicked, Some(Part::Changes), window, cx);
                    }))
            })
            .child(body)
    }

    fn home_agents(&self, look: &Look, card: &Card, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = look.theme;
        let rows = self.overview_rows(cx);
        let mine = model::rows_of(&rows, &card.endpoint, &card.workspace);
        let count = (!mine.is_empty()).then(|| mine.len().to_string());
        let mut list = div().flex().flex_col().gap_1();
        if mine.is_empty() {
            list = list.child(quiet(look, "No agent running"));
        }
        for (index, row) in mine.into_iter().enumerate() {
            let (clicked, folder) = (row.clone(), card.clone());
            list = list.child(
                div()
                    .id(("folder-home-agent", index))
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .px_1()
                    .py_0p5()
                    .rounded(px(corners::SMALL))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(theme.active)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_folder_agent(&folder, &clicked, window, cx);
                    }))
                    .child(crate::icons::icon(
                        AgentIcon::from_identity(row.identity.as_deref()).path(),
                        14.,
                        theme.foreground,
                    ))
                    .child(look.dot(row.state.status()))
                    .child(div().flex_1().min_w_0().truncate().child(row.title.clone()))
                    .child(
                        div()
                            .flex_none()
                            .max_w(px(140.))
                            .truncate()
                            .text_size(look.small())
                            .text_color(rgb(theme.muted))
                            .child(row.detail.clone()),
                    ),
            );
        }
        panel(look, "folder-home-agents", "Agents", count).child(list)
    }

    fn home_commits(
        &self,
        look: &Look,
        card: &Card,
        checkout: Option<&Checkout>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = look.theme;
        let now = SystemTime::now();
        let count = (card.ahead > 0).then(|| format!("↑{} to push", card.ahead));
        let commits = checkout
            .map(|checkout| checkout.commits.as_slice())
            .unwrap_or_default();
        let body: AnyElement = if commits.is_empty() {
            quiet(
                look,
                if checkout.is_some() {
                    "No commits yet"
                } else {
                    "Commits are read on this computer's checkouts only, so far."
                },
            )
            .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(commits.iter().take(LISTED).map(|commit| {
                    let at = UNIX_EPOCH + Duration::from_secs(commit.time.max(0).unsigned_abs());
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .min_w_0()
                        .child(
                            div()
                                .flex_none()
                                .text_font(look.mono)
                                .text_size(px(look.mono.size - 1.))
                                .text_color(rgb(theme.primary()))
                                .child(commit.short().to_owned()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(commit.subject.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_size(look.small())
                                .text_color(rgb(theme.muted))
                                .child(crate::changes::age(
                                    now.duration_since(at).unwrap_or_default(),
                                )),
                        )
                }))
                .into_any_element()
        };
        let clicked = card.clone();
        panel(look, "folder-home-commits", "Recent commits", count)
            .when(!commits.is_empty(), |panel| {
                panel
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(mix(theme.surface, theme.foreground, 4))))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_folder(&clicked, Some(Part::Changes), window, cx);
                    }))
            })
            .child(body)
    }

    fn home_ports(&self, look: &Look, card: &Card, cx: &mut Context<Self>) -> Stateful<Div> {
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
                    look.theme,
                    look.ui.size - 1.,
                    cx,
                )
            });
        let body = match ports {
            Some(ports) => ports.into_any_element(),
            None => quiet(look, "No ports open").into_any_element(),
        };
        panel(look, "folder-home-ports", "Running here", None).child(body)
    }
}

/// A card of the Home: a small title and count over its body.
fn panel(
    look: &Look,
    id: &'static str,
    title: &'static str,
    count: Option<String>,
) -> Stateful<Div> {
    let theme = look.theme;
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap_2()
        .p_3()
        .rounded(px(corners::PANEL))
        .bg(rgb(theme.surface))
        .border_1()
        .border_color(rgb(mix(theme.surface, theme.foreground, 8)))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_size(look.small())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme.subtext()))
                .child(title.to_uppercase())
                .when_some(count, |head, count| {
                    head.child(div().text_color(rgb(theme.muted)).child(count))
                }),
        )
}

fn quiet(look: &Look, text: &'static str) -> Div {
    div().text_color(rgb(look.theme.muted)).child(text)
}

fn more(look: &Look, total: usize) -> Option<Div> {
    (total > LISTED).then(|| {
        div()
            .text_size(look.small())
            .text_color(rgb(look.theme.muted))
            .child(format!("and {} more", total - LISTED))
    })
}

fn button(look: &Look, id: &'static str, label: &'static str, primary: bool) -> Stateful<Div> {
    let theme = look.theme;
    let fill = if primary {
        theme.primary()
    } else {
        theme.active
    };
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex_none()
        .px_2p5()
        .py_1()
        .rounded(px(corners::CONTROL))
        .cursor_pointer()
        .bg(rgb(fill))
        .text_color(rgb(theme.text_on(fill)))
        .text_size(look.small())
        .font_weight(FontWeight::SEMIBOLD)
        .child(label)
}
