//! A folder's pull requests and issues on its Home, and the checks on its
//! default branch, from what the Pull requests and Issues sections' view
//! last synced for its repository. A row opens the item's page in its
//! section; the title opens the section narrowed to the project.

use super::{LISTED, more, panel, quiet};
use crate::fonts::StyledFont;
use crate::{
    HerdrWindow,
    changes::age,
    config::corners,
    devices_overview::Look,
    folders::model::Card,
    orchestrator::{BranchChecks, CheckState, Item, PrStatus, Review, closed},
    sidebar::ActivityPanel,
};
use gpui::{prelude::*, *};
use std::time::SystemTime;

/// What the sections' view holds of a folder's repository, copied out of
/// the view for one draw.
pub(super) struct Github {
    /// The newest [`LISTED`] open pull requests, and how many are open.
    pull_requests: Vec<(Item, Option<PrStatus>)>,
    open_pull_requests: usize,
    /// The newest [`LISTED`] open issues, and how many are open.
    issues: Vec<Item>,
    open_issues: usize,
    /// Of the open pull requests, those asking the login's review.
    yours: usize,
    pub(super) default_checks: Option<BranchChecks>,
    /// The GitHub login the repository is read as.
    login: Option<String>,
    /// Whether a first listing finished.
    synced: bool,
}

/// Newest change first; one never changed after.
fn newest_first(a: &Item, b: &Item) -> std::cmp::Ordering {
    b.updated_at.cmp(&a.updated_at)
}

fn updated(look: &Look, item: &Item) -> Option<Div> {
    let at = item.updated_at.or(item.created_at)?;
    let since = SystemTime::from(at).elapsed().unwrap_or_default();
    Some(
        div()
            .flex_none()
            .text_size(look.small())
            .text_color(rgb(look.theme.muted))
            .child(age(since)),
    )
}

fn badge(look: &Look, text: &'static str, color: u32) -> Div {
    div()
        .flex_none()
        .px_1p5()
        .rounded(px(corners::SMALL))
        .border_1()
        .border_color(rgb(color))
        .text_color(rgb(color))
        .text_size(px(look.ui.size - 2.))
        .child(text)
}

impl HerdrWindow {
    /// What the sections' view last read of `card`'s repository, when it
    /// lists it.
    pub(super) fn folder_github(&self, card: &Card, cx: &App) -> Option<Github> {
        let key = self.folder_repo_key(card.endpoint_index, &card.workspace)?;
        let view = self.section_view()?;
        let view = view.read(cx);
        let (snapshot, login) = view.home_repo(&key)?;
        let status = |item: &Item| {
            let number = item.key.native_id.strip_prefix("pr/")?.parse().ok()?;
            snapshot
                .pr_status
                .get(&(item.key.source.repository.clone(), number))
        };
        let mut pull_requests: Vec<&Item> = Vec::new();
        let mut issues: Vec<&Item> = Vec::new();
        for item in snapshot.items.iter() {
            match &item.pull_request {
                Some(_) if matches!(item.state.as_str(), "open" | "draft") => {
                    pull_requests.push(item);
                }
                None if !closed(item) => issues.push(item),
                _ => {}
            }
        }
        pull_requests.sort_by(|a, b| newest_first(a, b));
        issues.sort_by(|a, b| newest_first(a, b));
        let yours = login.map_or(0, |login| {
            pull_requests
                .iter()
                .filter(|item| status(item).is_some_and(|status| status.requests(login)))
                .count()
        });
        let default_checks = snapshot
            .repo
            .as_ref()
            .and_then(|repo| repo.remote.as_ref())
            .and_then(|remote| snapshot.default_checks.get(&remote.source.repository))
            .cloned();
        Some(Github {
            open_pull_requests: pull_requests.len(),
            pull_requests: pull_requests
                .into_iter()
                .take(LISTED)
                .map(|item| (item.clone(), status(item).cloned()))
                .collect(),
            open_issues: issues.len(),
            issues: issues.into_iter().take(LISTED).cloned().collect(),
            yours,
            default_checks,
            login: login.map(str::to_owned),
            synced: !snapshot.sources.is_empty(),
        })
    }

    /// Shows `panel` narrowed to `card`'s project, with item `key`'s page
    /// open when given.
    fn open_folder_section(
        &mut self,
        card: &Card,
        panel: ActivityPanel,
        key: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activity.section_project = self.folder_repo_key(card.endpoint_index, &card.workspace);
        self.show_activity_panel(panel, window, cx);
        self.sync_activity_section(cx);
        if let (Some(key), Some(view)) = (key, self.section_view()) {
            view.update(cx, |view, cx| view.open_item(key, cx));
        }
    }

    pub(super) fn home_pull_requests(
        &self,
        look: &Look,
        card: &Card,
        github: Option<&Github>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = look.theme;
        let Some(github) = github else {
            return panel(look, "folder-home-pull-requests", "Pull requests", None)
                .child(quiet(look, self.folder_github_missing()));
        };
        let count = (github.open_pull_requests > 0).then(|| {
            let mut count = github.open_pull_requests.to_string();
            if github.yours > 0 {
                count.push_str(&format!(" · {} need your review", github.yours));
            }
            count
        });
        let mut list = div().flex().flex_col().gap_1();
        if github.pull_requests.is_empty() {
            list = list.child(quiet(
                look,
                if github.synced {
                    "No open pull requests"
                } else {
                    "Reading…"
                },
            ));
        }
        for (index, (item, status)) in github.pull_requests.iter().enumerate() {
            let checks =
                status
                    .as_ref()
                    .and_then(PrStatus::checks_state)
                    .map(|state| match state {
                        CheckState::Failed => theme.ink(theme.palette[1]),
                        CheckState::Passed => theme.ink(theme.palette[2]),
                        CheckState::Pending => theme.ink(theme.palette[3]),
                        CheckState::Skipped => theme.muted,
                    });
            let review = status.as_ref().and_then(|status| {
                if github
                    .login
                    .as_deref()
                    .is_some_and(|login| status.requests(login))
                {
                    return Some(("your review", theme.ink(theme.palette[5])));
                }
                Some(match status.review? {
                    Review::Approved => ("approved", theme.ink(theme.palette[2])),
                    Review::ChangesRequested => ("changes", theme.ink(theme.palette[1])),
                    Review::Required => ("review", theme.ink(theme.palette[4])),
                })
            });
            let (key, folder) = (item.key.canonical(), card.clone());
            list = list.child(
                row(look, ("folder-home-pull-request", index), item)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_folder_section(
                            &folder,
                            ActivityPanel::PullRequests,
                            Some(key.clone()),
                            window,
                            cx,
                        );
                    }))
                    .when(item.state == "draft", |row| {
                        row.child(badge(look, "draft", theme.muted))
                    })
                    .when_some(review, |row, (text, color)| {
                        row.child(badge(look, text, color))
                    })
                    .child(
                        div()
                            .flex_none()
                            .size(px(7.))
                            .rounded_full()
                            .bg(rgb(checks.unwrap_or(theme.active))),
                    )
                    .children(updated(look, item)),
            );
        }
        let folder = card.clone();
        panel(look, "folder-home-pull-requests", "Pull requests", count)
            .child(list.children(more(look, github.open_pull_requests)))
            .child(section_link(
                look,
                "folder-home-all-pull-requests",
                cx.listener(move |this, _, window, cx| {
                    this.open_folder_section(
                        &folder,
                        ActivityPanel::PullRequests,
                        None,
                        window,
                        cx,
                    );
                }),
            ))
    }

    pub(super) fn home_issues(
        &self,
        look: &Look,
        card: &Card,
        github: Option<&Github>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = look.theme;
        let Some(github) = github else {
            return panel(look, "folder-home-issues", "Issues", None)
                .child(quiet(look, self.folder_github_missing()));
        };
        let count = (github.open_issues > 0).then(|| github.open_issues.to_string());
        let mut list = div().flex().flex_col().gap_1();
        if github.issues.is_empty() {
            list = list.child(quiet(
                look,
                if github.synced {
                    "No open issues"
                } else {
                    "Reading…"
                },
            ));
        }
        for (index, item) in github.issues.iter().enumerate() {
            let (key, folder) = (item.key.canonical(), card.clone());
            list = list.child(
                row(look, ("folder-home-issue", index), item)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_folder_section(
                            &folder,
                            ActivityPanel::Issues,
                            Some(key.clone()),
                            window,
                            cx,
                        );
                    }))
                    .children(item.labels.first().map(|label| {
                        div()
                            .flex_none()
                            .max_w(px(110.))
                            .truncate()
                            .text_size(look.small())
                            .text_color(rgb(theme.muted))
                            .child(label.clone())
                    }))
                    .children(updated(look, item)),
            );
        }
        let folder = card.clone();
        panel(look, "folder-home-issues", "Issues", count)
            .child(list.children(more(look, github.open_issues)))
            .child(section_link(
                look,
                "folder-home-all-issues",
                cx.listener(move |this, _, window, cx| {
                    this.open_folder_section(&folder, ActivityPanel::Issues, None, window, cx);
                }),
            ))
    }

    /// Why a folder's pull requests and issues are not listed.
    fn folder_github_missing(&self) -> &'static str {
        let listed = [ActivityPanel::PullRequests, ActivityPanel::Issues]
            .into_iter()
            .any(|panel| self.activity_panel_listed(panel));
        if !self.activity_bar_shown() || !listed {
            "Listed when the Pull requests or Issues section is on"
        } else {
            "Not a repository the sections list"
        }
    }
}

/// An item's row: its number or id, then its title; the caller adds marks.
fn row(look: &Look, id: (&'static str, usize), item: &Item) -> Stateful<Div> {
    let theme = look.theme;
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .min_w_0()
        .px_1()
        .py_0p5()
        .rounded(px(corners::SMALL))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(theme.active)))
        .child(
            div()
                .flex_none()
                .text_font(look.mono)
                .text_size(px(look.mono.size - 1.))
                .text_color(rgb(theme.primary()))
                .child(item.identifier.clone()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(item.title.clone()),
        )
}

/// "Open in its section", at a card's foot.
fn section_link(
    look: &Look,
    id: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let hovered = look.theme.foreground;
    div()
        .id(id)
        .debug_selector(move || id.into())
        .mt_auto()
        .text_size(look.small())
        .text_color(rgb(look.theme.muted))
        .cursor_pointer()
        .hover(move |style| style.text_color(rgb(hovered)))
        .on_click(on_click)
        .child("Show all →")
}
