//! Where a folder's repository stands on its Home: its recent branches, its
//! other worktrees, its sync with the remote and its latest release, and
//! its activity over the last days. The worktrees come from the Changes
//! reader; the rest from the overview `read` keeps.

use super::{github::Github, panel, quiet};
use crate::fonts::StyledFont;
use crate::{
    HerdrWindow,
    changes::age,
    config::corners,
    devices_overview::Look,
    folders::{Part, model::Card},
    orchestrator::CheckState,
};
use gpui::{prelude::*, *};
use herdr_vcs::changes::{Checkout, overview::Overview};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Tallest bar of the activity strip.
const STRIP: f32 = 36.;

/// How long ago `time` (Unix seconds) was, as the Changes section says it.
fn ago(time: i64) -> String {
    let at = UNIX_EPOCH + Duration::from_secs(time.max(0).unsigned_abs());
    age(SystemTime::now().duration_since(at).unwrap_or_default())
}

/// Why a card that reads the overview has nothing yet.
fn unread(look: &Look, card: &Card) -> Div {
    quiet(
        look,
        match (&card.checkout, &card.host) {
            (Some(_), _) => "Reading…",
            (None, Some(_)) => "Read on this computer's checkouts only, so far.",
            (None, None) => "Not a Git checkout",
        },
    )
}

fn small(look: &Look, color: u32, text: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .text_size(look.small())
        .text_color(rgb(color))
        .child(text.into())
}

fn dot(color: u32) -> Div {
    div().flex_none().size(px(7.)).rounded_full().bg(rgb(color))
}

/// A card's line: a fixed-width label, then what it says.
fn line(look: &Look, label: &'static str, value: impl IntoElement) -> Div {
    div()
        .flex()
        .items_center()
        .gap_2()
        .min_w_0()
        .child(
            div()
                .flex_none()
                .w(px(76.))
                .text_size(look.small())
                .text_color(rgb(look.theme.muted))
                .child(label),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .flex_1()
                .min_w_0()
                .child(value),
        )
}

pub(super) fn home_branches(
    look: &Look,
    card: &Card,
    overview: Option<&Overview>,
) -> Stateful<Div> {
    let theme = look.theme;
    let Some(overview) = overview else {
        return panel(look, "folder-home-branches", "Branches", None).child(unread(look, card));
    };
    let unmerged = overview
        .branches
        .iter()
        .filter(|branch| branch.ahead_behind.is_some() && !branch.merged())
        .count();
    let count = (unmerged > 0).then(|| format!("{unmerged} not merged"));
    let rows = overview.branches.iter().enumerate().map(|(index, branch)| {
        let state = match branch.ahead_behind {
            Some((0, _)) => small(look, theme.muted, "merged"),
            Some((ahead, behind)) => {
                let mut text = format!("↑{ahead}");
                if behind > 0 {
                    text.push_str(&format!(" ↓{behind}"));
                }
                small(look, theme.ink(theme.palette[3]), text)
            }
            None => div(),
        };
        div()
            .debug_selector(move || format!("folder-home-branch-{index}"))
            .flex()
            .items_center()
            .gap_2()
            .min_w_0()
            .child(
                div()
                    .flex_none()
                    .max_w(px(220.))
                    .truncate()
                    .text_font(look.mono)
                    .text_size(px(look.mono.size - 1.))
                    .text_color(rgb(if branch.current {
                        theme.primary()
                    } else {
                        theme.foreground
                    }))
                    .child(branch.name.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(rgb(theme.muted))
                    .child(branch.subject.clone()),
            )
            .child(state)
            .child(small(look, theme.muted, ago(branch.time)))
    });
    panel(look, "folder-home-branches", "Branches", count).child(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .children(rows)
            .when(overview.branches.is_empty(), |list| {
                list.child(quiet(look, "No branches yet"))
            }),
    )
}

pub(super) fn home_sync(
    look: &Look,
    card: &Card,
    checkout: Option<&Checkout>,
    overview: Option<&Overview>,
    github: Option<&Github>,
) -> Stateful<Div> {
    let theme = look.theme;
    let (green, red, yellow) = (
        theme.ink(theme.palette[2]),
        theme.ink(theme.palette[1]),
        theme.ink(theme.palette[3]),
    );
    let mut body = div().flex().flex_col().gap_1p5();
    let Some(checkout) = checkout else {
        return panel(look, "folder-home-sync", "Sync", None).child(unread(look, card));
    };
    let remote = match (&checkout.upstream, checkout.ahead, checkout.behind) {
        (None, ..) => small(look, theme.muted, "No upstream"),
        (Some(upstream), 0, 0) => small(look, green, format!("Up to date with {upstream}")),
        (Some(upstream), ahead, behind) => {
            let mut parts = Vec::new();
            if ahead > 0 {
                parts.push(format!("↑{ahead} to push"));
            }
            if behind > 0 {
                parts.push(format!("↓{behind} to pull"));
            }
            small(look, yellow, format!("{} · {upstream}", parts.join(" ")))
        }
    };
    body = body.child(line(look, "Upstream", remote));
    if let Some(unmerged) = checkout
        .unmerged
        .as_ref()
        .filter(|unmerged| unmerged.count > 0)
    {
        body = body.child(line(
            look,
            "Unmerged",
            small(
                look,
                yellow,
                format!(
                    "{} {} not on {}",
                    unmerged.count,
                    if unmerged.count == 1 {
                        "commit"
                    } else {
                        "commits"
                    },
                    unmerged.base
                ),
            ),
        ));
    }
    if let Some((base, moved)) =
        overview.and_then(|overview| overview.base.as_ref().zip(overview.base_moved))
    {
        body = body.child(line(
            look,
            "Fetched",
            small(
                look,
                theme.foreground,
                format!("{base} moved {}", ago(moved)),
            ),
        ));
    }
    let ci = github.and_then(|github| github.default_checks.as_ref());
    body = body.child(line(
        look,
        "CI",
        match ci {
            Some(checks) => {
                let failing: Vec<&str> = checks
                    .checks
                    .iter()
                    .filter(|check| check.state == CheckState::Failed)
                    .map(|check| check.name.as_str())
                    .collect();
                let (color, text) = match checks.checks_state() {
                    Some(CheckState::Failed) => (
                        red,
                        format!(
                            "{} failing on {}: {}",
                            failing.len(),
                            checks.branch,
                            failing.join(", ")
                        ),
                    ),
                    Some(CheckState::Pending) => (yellow, format!("Running on {}", checks.branch)),
                    Some(CheckState::Passed) => (green, format!("Passing on {}", checks.branch)),
                    Some(CheckState::Skipped) | None => {
                        (theme.muted, format!("No checks on {}", checks.branch))
                    }
                };
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .child(dot(color))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(look.small())
                            .child(text),
                    )
            }
            None => small(
                look,
                theme.muted,
                if github.is_some() {
                    "Not read yet"
                } else {
                    "Read with the Pull requests section"
                },
            ),
        },
    ));
    if let Some(overview) = overview {
        let release = match &overview.release {
            Some(release) => small(
                look,
                if release.since > 0 {
                    yellow
                } else {
                    theme.foreground
                },
                format!(
                    "{} · {} since · {}",
                    release.tag,
                    match release.since {
                        0 => "nothing".to_owned(),
                        1 => "1 commit".to_owned(),
                        since => format!("{since} commits"),
                    },
                    ago(release.time)
                ),
            ),
            None => small(look, theme.muted, "No tags yet"),
        };
        body = body.child(line(look, "Release", release));
    }
    panel(look, "folder-home-sync", "Sync", None).child(body)
}

pub(super) fn home_activity(
    look: &Look,
    card: &Card,
    overview: Option<&Overview>,
) -> Stateful<Div> {
    let theme = look.theme;
    let Some(overview) = overview else {
        return panel(look, "folder-home-activity", "Activity", None).child(unread(look, card));
    };
    let total: u32 = overview.days.iter().sum();
    let most = overview
        .days
        .iter()
        .copied()
        .max()
        .unwrap_or_default()
        .max(1);
    let count = Some(format!(
        "{total} {} in {} days",
        if total == 1 { "commit" } else { "commits" },
        overview.days.len()
    ));
    let bars = overview.days.iter().map(|&commits| {
        let height = if commits == 0 {
            2.
        } else {
            (STRIP * commits as f32 / most as f32).max(4.)
        };
        div()
            .flex_1()
            .h(px(height))
            .rounded(px(corners::SMALL))
            .bg(rgb(if commits == 0 {
                theme.active
            } else {
                theme.primary()
            }))
    });
    let hot = overview.hot.iter().map(|hot| {
        div()
            .flex()
            .items_center()
            .gap_2()
            .min_w_0()
            .text_font(look.mono)
            .text_size(px(look.mono.size - 1.))
            .child(div().flex_1().min_w_0().truncate().child(hot.path.clone()))
            .child(small(look, theme.muted, format!("{}×", hot.commits)))
    });
    panel(look, "folder-home-activity", "Activity", count)
        .child(div().flex().items_end().gap_1().h(px(STRIP)).children(bars))
        .when(!overview.hot.is_empty(), |panel| {
            panel
                .child(small(look, theme.muted, "Most changed"))
                .child(div().flex().flex_col().gap_1().children(hot))
        })
}

impl HerdrWindow {
    /// The repository's other checkouts, as the Changes reader found them;
    /// one with a workspace open shows that folder's Home.
    pub(super) fn home_worktrees(
        &self,
        look: &Look,
        card: &Card,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = look.theme;
        let Some(path) = card.checkout.as_deref() else {
            return panel(look, "folder-home-worktrees", "Worktrees", None)
                .child(unread(look, card));
        };
        let others: Vec<_> = self
            .changes
            .repos()
            .iter()
            .find(|repo| {
                repo.checkouts
                    .iter()
                    .any(|shown| shown.checkout.path == path)
            })
            .map(|repo| {
                repo.checkouts
                    .iter()
                    .filter(|shown| shown.checkout.path != path)
                    .collect()
            })
            .unwrap_or_default();
        let count = (!others.is_empty()).then(|| others.len().to_string());
        let mut list = div().flex().flex_col().gap_1();
        if others.is_empty() {
            list = list.child(quiet(look, "Only this checkout"));
        }
        let cards = if others.is_empty() {
            Vec::new()
        } else {
            self.folder_cards(cx)
        };
        for (index, shown) in others.into_iter().enumerate() {
            let checkout = &shown.checkout;
            let mut state = Vec::new();
            if checkout.dirty() {
                state.push((
                    format!("{} uncommitted", checkout.total_files),
                    theme.ink(theme.palette[3]),
                ));
            }
            if checkout.ahead > 0 {
                state.push((format!("↑{}", checkout.ahead), theme.ink(theme.palette[3])));
            }
            match checkout.unmerged.as_ref() {
                Some(unmerged) if unmerged.count > 0 => state.push((
                    format!("{} unmerged", unmerged.count),
                    theme.ink(theme.palette[1]),
                )),
                _ => {}
            }
            if state.is_empty() {
                state.push(("clean".into(), theme.muted));
            }
            // The Changes reader reads this machine's own daemon only.
            let open = shown.workspaces.first().and_then(|workspace| {
                cards
                    .iter()
                    .find(|other| other.endpoint_index == 0 && other.workspace == workspace.id)
                    .cloned()
            });
            let name = std::path::Path::new(&checkout.path)
                .file_name()
                .map_or_else(
                    || checkout.path.clone(),
                    |name| name.to_string_lossy().into_owned(),
                );
            list = list.child(
                div()
                    .id(("folder-home-worktree", index))
                    .debug_selector(move || format!("folder-home-worktree-{index}"))
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .px_1()
                    .py_0p5()
                    .rounded(px(corners::SMALL))
                    .when_some(open, |row, open| {
                        row.cursor_pointer()
                            .hover(|style| style.bg(rgb(theme.active)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_folder(&open, Some(Part::Home), window, cx);
                            }))
                    })
                    .child(
                        div()
                            .flex_none()
                            .max_w(px(200.))
                            .truncate()
                            .text_font(look.mono)
                            .text_size(px(look.mono.size - 1.))
                            .child(checkout.branch.clone().unwrap_or_else(|| "detached".into())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(look.small())
                            .text_color(rgb(theme.muted))
                            .child(name),
                    )
                    .children(
                        state
                            .into_iter()
                            .map(|(text, color)| small(look, color, text)),
                    ),
            );
        }
        panel(look, "folder-home-worktrees", "Worktrees", count).child(list)
    }
}
