//! A folder's own work on its Home: its uncommitted files and its latest
//! commits, as the Changes reader read them. Either opens its Changes.

use super::{LISTED, more, panel, quiet};
use crate::fonts::StyledFont;
use crate::{
    HerdrWindow,
    config::mix,
    devices_overview::Look,
    folders::{Part, model::Card},
};
use gpui::{prelude::*, *};
use herdr_vcs::changes::{Checkout, Kind};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

impl HerdrWindow {
    pub(super) fn home_uncommitted(
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

    pub(super) fn home_commits(
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
}
