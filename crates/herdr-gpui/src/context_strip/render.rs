//! Drawing the strip: segments of an icon and a value split by hairlines,
//! the checkout's on the left and its pull request on the right. A problem
//! colors only its own segment. Segments that lead somewhere take a click:
//! Git's open the Changes section, the pull request opens on GitHub.

use super::Target;
use crate::{
    HerdrWindow,
    changes::{Rank, home_relative, rank},
    config::{Theme, corners, mix},
    pull_request::Outcome,
    sidebar::ActivityPanel,
};
use gpui::{prelude::*, *};
use herdr_vcs::context::Context as Read;
use std::path::Path;

const ICON: f32 = 13.;

impl HerdrWindow {
    /// The strip's height, which grows with the UI font.
    pub(crate) fn context_strip_height(&self) -> f32 {
        (self.config.ui.size * 1.4 + 4.).max(20.)
    }

    /// The strip, while it is on and a pane is focused; never in zen mode.
    pub(crate) fn render_context_strip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.config.show_context_strip || self.zen_layout() {
            return None;
        }
        let target = self.context_strip.target.as_ref()?;
        let theme = &self.theme;
        let strip = div()
            .id("context-strip")
            .debug_selector(|| "context-strip".into())
            .role(Role::Group)
            .aria_label("Context strip")
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(px(self.context_strip_height()))
            .px_2()
            .overflow_hidden()
            .whitespace_nowrap()
            .border_t_1()
            .border_color(rgb(mix(theme.background, theme.foreground, 10)))
            .bg(self.fill().panel(mix(theme.background, theme.surface, 50)))
            .text_size(px(self.config.ui.size - 1.))
            .text_color(rgb(theme.subtext()));
        let folder = |note: Option<&'static str>| {
            let path = if target.local {
                home_relative(&target.directory)
            } else {
                target.directory.clone()
            };
            div()
                .flex()
                .min_w_0()
                .gap_2()
                .text_color(rgb(theme.muted))
                .child(div().min_w_0().truncate().child(path))
                .children(note.map(|note| div().flex_none().child(note)))
        };
        Some(match &self.context_strip.read {
            _ if !target.local => strip
                .child(folder(Some("Git details are read on this machine only")))
                .into_any_element(),
            None => strip.child(folder(None)).into_any_element(),
            Some(Err(_)) => strip
                .child(folder(Some("not a Git checkout")))
                .into_any_element(),
            Some(Ok(read)) => strip
                .child(self.context_segments(target, read, cx))
                .child(div().flex_1())
                .children(self.context_pull_request(target, read, cx))
                .into_any_element(),
        })
    }

    /// The checkout's segments, hairlines between them. They clip, path and
    /// branch name first, so the pull request on the right keeps its room.
    fn context_segments(&self, target: &Target, read: &Read, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        let checkout = &read.checkout;
        let (red, green, yellow) = (ink(theme, 1), ink(theme, 2), ink(theme, 3));
        let mut segments: Vec<AnyElement> = Vec::new();

        let (path, inside) = repo_path(&checkout.path, &target.directory);
        let label = format!("{path}{inside}");
        let quiet = HighlightStyle {
            color: Some(rgb(theme.muted).into()),
            ..Default::default()
        };
        segments.push(
            segment("path", label.clone())
                .flex_shrink(1.)
                .min_w_0()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(rgb(theme.subtext()))
                        .child(
                            StyledText::new(label.clone())
                                .with_highlights([(path.len()..label.len(), quiet)]),
                        ),
                )
                .into_any_element(),
        );

        let branch = checkout.branch.as_deref().unwrap_or("detached HEAD");
        segments.push(
            self.to_changes(segment("branch", branch), cx)
                .flex_shrink(1.)
                .min_w_0()
                .child(icon("icons/git-branch.svg", theme.muted))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(rgb(theme.foreground))
                        .child(branch.to_owned()),
                )
                .when(checkout.ahead > 0, |el| {
                    el.child(text(format!("↑{}", checkout.ahead), green))
                })
                .when(checkout.behind > 0, |el| {
                    el.child(text(format!("↓{}", checkout.behind), yellow))
                })
                .into_any_element(),
        );
        if let Some(stopped) = read.state.stopped {
            let label = format!("{} stopped", stopped.label());
            segments.push(
                segment("stopped", label.clone())
                    .child(icon("icons/error.svg", red))
                    .child(text(label, red))
                    .into_any_element(),
            );
        }
        segments.push(if checkout.dirty() {
            let files = checkout.total_files;
            let (added, removed) = checkout.lines();
            let label = format!("{files} {}", plural(files, "file", "files"));
            self.to_changes(segment("changes", label.clone()), cx)
                .child(icon("icons/pencil.svg", yellow))
                .child(label)
                .when(added > 0, |el| el.child(text(format!("+{added}"), green)))
                .when(removed > 0, |el| el.child(text(format!("−{removed}"), red)))
                .into_any_element()
        } else {
            self.to_changes(segment("changes", "clean"), cx)
                .child(icon("icons/check.svg", green))
                .child(text("clean", theme.muted))
                .into_any_element()
        });
        if read.state.stashes > 0 {
            let label = format!("stash {}", read.state.stashes);
            segments.push(
                segment("stash", label.clone())
                    .child(text(label, theme.muted))
                    .into_any_element(),
            );
        }
        if !read.worktrees.is_empty() {
            let others = read.worktrees.len();
            let pending = pending_worktrees(read);
            let label = format!("{others} other {}", plural(others, "worktree", "worktrees"));
            segments.push(
                self.to_changes(segment("worktrees", label.clone()), cx)
                    .child(icon(
                        "icons/split.svg",
                        if pending > 0 { yellow } else { theme.muted },
                    ))
                    .child(label)
                    .when(pending > 0, |el| {
                        el.child(text(format!("· {pending} not merged"), yellow))
                    })
                    .into_any_element(),
            );
        }
        if read.state.crypt_locked {
            segments.push(
                segment("crypt", "git-crypt locked")
                    .child(icon("icons/lock.svg", red))
                    .child(text("git-crypt locked", red))
                    .into_any_element(),
            );
        }

        let rule = mix(theme.background, theme.foreground, 18);
        let mut row = div()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap_1()
            .overflow_hidden();
        for (index, segment) in segments.into_iter().enumerate() {
            if index > 0 {
                row = row.child(div().flex_none().w(px(1.)).h(px(12.)).bg(rgb(rule)));
            }
            row = row.child(segment);
        }
        row
    }

    /// The branch's pull request and its checks, from the prefetch's cache;
    /// nothing until it has one.
    fn context_pull_request(
        &self,
        target: &Target,
        read: &Read,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let key = target.repo_key.as_deref()?;
        let branch = read.checkout.branch.as_deref()?;
        let view = self.endpoint_pr_view(0)?;
        let pr = self.menu.pr_cache.peek(&view, key, branch)?;
        let theme = &self.theme;
        let checks = pr.checks_outcome().and_then(|outcome| match outcome {
            Outcome::Failed => Some(("CI failed", ink(theme, 1))),
            Outcome::Pending => Some(("CI running", ink(theme, 3))),
            Outcome::Passed => Some(("CI passed", ink(theme, 2))),
            Outcome::Skipped => None,
        });
        let url = pr.url.clone();
        let number = format!("#{}", pr.number);
        Some(
            segment("pull-request", format!("Pull request {number}"))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(theme.active)))
                .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url)))
                .child(icon(
                    "icons/pull-request.svg",
                    crate::pull_request::color(&pr, theme),
                ))
                .child(number)
                .children(checks.map(|(word, color)| text(word, color))),
        )
    }

    /// Makes `segment` open the Changes section, when the bar lists it.
    fn to_changes(&self, segment: Stateful<Div>, cx: &mut Context<Self>) -> Stateful<Div> {
        if !self.activity_panel_listed(ActivityPanel::Changes) {
            return segment;
        }
        let hover = self.theme.active;
        segment
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(hover)))
            .on_click(cx.listener(|this, _, window, cx| {
                this.show_activity_panel(ActivityPanel::Changes, window, cx);
            }))
    }
}

/// The checkout's top folder with home written `~`, and where the pane is
/// below it, if deeper: `("~/code/app", "/crates/core")`.
pub(super) fn repo_path(checkout: &str, directory: &str) -> (String, String) {
    let inside = Path::new(directory)
        .strip_prefix(checkout)
        .ok()
        .filter(|rest| !rest.as_os_str().is_empty())
        .map(|rest| format!("/{}", rest.display()))
        .unwrap_or_default();
    (home_relative(checkout), inside)
}

/// The other worktrees holding work not on the default branch yet:
/// uncommitted, unpushed, or unmerged.
pub(super) fn pending_worktrees(read: &Read) -> usize {
    read.worktrees
        .iter()
        .filter(|worktree| rank(worktree) < Rank::Behind)
        .count()
}

fn segment(id: &'static str, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || format!("context-{id}"))
        .aria_label(label.into())
        .flex()
        .flex_none()
        .items_center()
        .gap_1()
        .px(px(4.))
        .rounded(px(corners::SMALL))
}

fn icon(path: &'static str, color: u32) -> Svg {
    svg()
        .flex_none()
        .size(px(ICON))
        .path(path)
        .text_color(rgb(color))
}

fn text(value: impl Into<SharedString>, color: u32) -> Div {
    div().flex_none().text_color(rgb(color)).child(value.into())
}

/// An ANSI hue inked to read on the chrome.
fn ink(theme: &Theme, index: usize) -> u32 {
    theme.ink(theme.palette[index])
}

fn plural(count: usize, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 { one } else { many }
}
