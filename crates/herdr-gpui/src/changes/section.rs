//! Drawing the Changes section from the last read: a card per repository,
//! and in it each checkout's branch, its uncommitted files (`files`), and
//! its latest commits. A checkout's header opens the workspace open in it, if any.
//! Needs attention, the default, folds away the clean checkouts.

use super::{Rank, Repo, Shown};
use crate::HerdrWindow;
use gpui::{prelude::*, *};
use herdr_vcs::changes::Commit;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

impl HerdrWindow {
    pub(crate) fn render_changes_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.theme;
        let font = &self.config.ui;
        let small = px(font.size - 1.);
        let updated = self.changes.read_at().map(|at| {
            format!(
                "Updated {}",
                age(SystemTime::now().duration_since(at).unwrap_or_default())
            )
        });
        let header = div()
            .debug_selector(|| "changes-section-header".into())
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
                    .text_size(px(font.size + 3.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Changes"),
            )
            .child(
                div()
                    .text_size(small)
                    .text_color(rgb(theme.muted))
                    .child("Uncommitted work and recent commits on this machine"),
            )
            .child(div().flex_1())
            .children(updated.map(|updated| {
                div()
                    .text_size(small)
                    .text_color(rgb(theme.muted))
                    .child(updated)
            }))
            .child(self.changes_filter(cx))
            .child(
                div()
                    .id("changes-section-refresh")
                    .debug_selector(|| "changes-section-refresh".into())
                    .px(px(10.))
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .gap_1()
                    .rounded(px(crate::config::corners::CONTROL))
                    .border_1()
                    .border_color(rgb(theme.active))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(theme.active)))
                    .text_size(small)
                    .child(
                        svg()
                            .path("icons/refresh.svg")
                            .size(px(12.))
                            .text_color(rgb(theme.subtext())),
                    )
                    .child(if self.changes.busy() {
                        "Updating…"
                    } else {
                        "Refresh"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.changes.refresh();
                        this.poll_changes(cx);
                        cx.notify();
                    })),
            );
        div()
            .id("changes-section")
            .debug_selector(|| "changes-section".into())
            .flex()
            .flex_col()
            .flex_1()
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(rgb(theme.background))
            .text_color(rgb(theme.foreground))
            .text_size(px(font.size))
            .child(header)
            .child(
                div()
                    .id("changes-section-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.changes.scroll)
                    .child(self.changes_body(cx)),
            )
            .into_any_element()
    }

    /// Needs attention or All, as one control that names what it lists.
    fn changes_filter(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = &self.theme;
        let small = px(self.config.ui.size - 1.);
        let option = |label: &'static str, picked: bool| {
            div()
                .px(px(8.))
                .h_full()
                .flex()
                .items_center()
                .rounded(px(crate::config::corners::CONTROL))
                .when(picked, |el| {
                    el.bg(rgb(theme.active)).text_color(rgb(theme.foreground))
                })
                .when(!picked, |el| el.text_color(rgb(theme.muted)))
                .child(label)
        };
        div()
            .id("changes-section-filter")
            .debug_selector(|| "changes-section-filter".into())
            .h(px(24.))
            .p(px(2.))
            .flex()
            .rounded(px(crate::config::corners::CONTROL))
            .border_1()
            .border_color(rgb(theme.active))
            .cursor_pointer()
            .text_size(small)
            .child(option("Needs attention", !self.changes.all))
            .child(option("All", self.changes.all))
            .on_click(cx.listener(|this, _, _, cx| {
                this.changes.all = !this.changes.all;
                cx.notify();
            }))
    }

    fn changes_body(&self, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        let mut body = div().p_5().flex().flex_col().gap_5();
        let repos = self.changes.repos();
        let failed = self.changes.failed();
        if repos.is_empty() && failed.is_empty() {
            let text = if self.changes.busy() || self.changes.read_at().is_none() {
                "Reading the checkouts…"
            } else {
                "No workspace or pane on this machine is in a Git checkout, and \
                 [changes] names none."
            };
            return body.child(
                div()
                    .debug_selector(|| "changes-section-empty".into())
                    .text_color(rgb(theme.muted))
                    .child(text),
            );
        }
        let all = self.changes.all;
        let mut listed = false;
        for (index, repo) in repos.iter().enumerate() {
            let shown: Vec<&Shown> = repo
                .checkouts
                .iter()
                .filter(|shown| all || shown.rank() != Rank::Clean)
                .collect();
            if !shown.is_empty() {
                listed = true;
                body = body.child(self.changes_repo(index, repo, &shown, cx));
            }
        }
        let clean = self.changes.clean();
        if !all && clean > 0 {
            let noun = if clean == 1 { "checkout" } else { "checkouts" };
            let text = if listed {
                format!("{clean} clean {noun} hidden · Show all")
            } else {
                format!("Nothing needs attention: {clean} {noun}, all clean · Show all")
            };
            body = body.child(
                div()
                    .id("changes-section-clean")
                    .debug_selector(|| "changes-section-clean".into())
                    .cursor_pointer()
                    .text_color(rgb(theme.muted))
                    .hover(|style| style.text_color(rgb(theme.foreground)))
                    .child(text)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.changes.all = true;
                        cx.notify();
                    })),
            );
        }
        for failure in failed {
            body = body.child(
                div()
                    .debug_selector(|| "changes-section-failed".into())
                    .text_color(rgb(theme.ink(theme.palette[3])))
                    .child(format!("{}: {}", failure.label, failure.error)),
            );
        }
        body
    }

    fn changes_repo(
        &self,
        index: usize,
        repo: &Repo,
        shown: &[&Shown],
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = &self.theme;
        let small = px(self.config.ui.size - 1.);
        let dirty = repo
            .checkouts
            .iter()
            .filter(|shown| shown.checkout.dirty())
            .count();
        let summary = match (repo.checkouts.len(), dirty) {
            (1, _) => None,
            (count, 0) => Some(format!("{count} checkouts")),
            (count, dirty) => Some(format!("{count} checkouts · {dirty} with changes")),
        };
        let mut card = div()
            .flex()
            .flex_col()
            .rounded(px(crate::config::corners::PANEL))
            .bg(rgb(theme.surface));
        for (position, shown) in shown.iter().enumerate() {
            card = card.child(
                self.changes_checkout(index, position, shown, cx)
                    .when(position > 0, |el| {
                        el.border_t_1().border_color(rgb(theme.active))
                    }),
            );
        }
        div()
            .debug_selector(|| "changes-repo".into())
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(repo.name.clone()),
                    )
                    .children(summary.map(|summary| {
                        div()
                            .text_size(small)
                            .text_color(rgb(theme.muted))
                            .child(summary)
                    })),
            )
            .child(card)
    }

    fn changes_checkout(
        &self,
        repo: usize,
        position: usize,
        shown: &Shown,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = &self.theme;
        let small = px(self.config.ui.size - 1.);
        let checkout = &shown.checkout;
        let (green, red, yellow, blue) = (
            theme.ink(theme.palette[2]),
            theme.ink(theme.palette[1]),
            theme.ink(theme.palette[3]),
            theme.ink(theme.palette[4]),
        );
        let branch = checkout
            .branch
            .clone()
            .unwrap_or_else(|| "detached HEAD".into());
        // The workspaces open in it, else where it is: a worktree no pane
        // is in any more.
        let labels = if shown.workspaces.is_empty() {
            home_relative(&checkout.path)
        } else {
            shown
                .workspaces
                .iter()
                .map(|workspace| workspace.label.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let state = match super::rank(checkout) {
            Rank::Dirty => {
                let (added, removed) = checkout.lines();
                let files = checkout.total_files;
                let noun = if files == 1 { "file" } else { "files" };
                div()
                    .text_color(rgb(yellow))
                    .child(format!("+{added} −{removed} · {files} {noun}"))
            }
            Rank::Unpushed | Rank::Unmerged | Rank::Behind | Rank::Clean => div()
                .text_color(rgb(theme.muted))
                .child("No uncommitted changes"),
        };
        let workspace = shown
            .workspaces
            .first()
            .map(|workspace| workspace.id.clone());
        let hover = theme.active;
        let header = div()
            .id(SharedString::from(format!(
                "changes-checkout-{repo}-{position}"
            )))
            .debug_selector(|| "changes-checkout".into())
            .px_3()
            .py(px(8.))
            .flex()
            .items_center()
            .gap_2()
            .when(workspace.is_some(), |el| {
                el.cursor_pointer().hover(move |style| style.bg(rgb(hover)))
            })
            .child(
                svg()
                    .path("icons/git-branch.svg")
                    .size(px(13.))
                    .flex_none()
                    .text_color(rgb(theme.subtext())),
            )
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(branch),
            )
            .when(checkout.ahead > 0, |el| {
                el.child(
                    div()
                        .flex_none()
                        .text_size(small)
                        .text_color(rgb(green))
                        .child(format!("↑{}", checkout.ahead)),
                )
            })
            .when_some(checkout.unmerged.as_ref(), |el, unmerged| {
                el.when(unmerged.count > 0, |el| {
                    el.child(
                        div()
                            .flex_none()
                            .text_size(small)
                            .text_color(rgb(blue))
                            .child(format!("{} not on {}", unmerged.count, unmerged.base)),
                    )
                })
            })
            .when(checkout.behind > 0, |el| {
                el.child(
                    div()
                        .flex_none()
                        .text_size(small)
                        .text_color(rgb(red))
                        .child(format!("↓{}", checkout.behind)),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(small)
                    .text_color(rgb(theme.muted))
                    .child(labels),
            )
            .child(div().flex_none().text_size(small).child(state))
            .on_click(cx.listener(move |this, _, window, cx| {
                if let Some(id) = &workspace {
                    this.open_changes_workspace(id, window, cx);
                }
            }));
        let mut block = div()
            .flex()
            .flex_col()
            .pb_2()
            .child(header)
            .children(self.changes_files(&format!("{repo}-{position}"), shown, cx));
        let more = checkout.total_files.saturating_sub(checkout.files.len());
        if more > 0 {
            block = block.child(
                div()
                    .px_3()
                    .pl(px(32.))
                    .py(px(2.))
                    .text_size(small)
                    .text_color(rgb(theme.muted))
                    .child(format!("and {more} more")),
            );
        }
        if !checkout.commits.is_empty() {
            block = block.child(div().h(px(6.)));
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        for commit in &checkout.commits {
            block = block.child(self.changes_commit(commit, now));
        }
        block
    }

    fn changes_commit(&self, commit: &Commit, now: u64) -> Div {
        let theme = &self.theme;
        let small = px(self.config.ui.size - 1.);
        let elapsed = now.saturating_sub(u64::try_from(commit.time).unwrap_or_default());
        div()
            .debug_selector(|| "changes-commit".into())
            .px_3()
            .pl(px(32.))
            .py(px(1.))
            .flex()
            .items_center()
            .gap_2()
            .text_size(small)
            .child(
                div()
                    .flex_none()
                    .text_color(rgb(theme.muted))
                    .child(commit.sha.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(rgb(theme.subtext()))
                    .child(commit.subject.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .text_color(rgb(theme.muted))
                    .child(format!(
                        "{} · {}",
                        commit.author,
                        age(Duration::from_secs(elapsed))
                    )),
            )
    }
}

/// `path` with the home folder written `~`.
fn home_relative(path: &str) -> String {
    herdr_client::local_path::home_dir()
        .and_then(|home| {
            let rest = std::path::Path::new(path).strip_prefix(&home).ok()?;
            Some(format!("~/{}", rest.display()))
        })
        .unwrap_or_else(|| path.to_owned())
}

/// How long ago, to the minute while recent and to the day once old: a
/// commit may be months old.
pub(super) fn age(elapsed: Duration) -> String {
    match elapsed.as_secs() / 60 {
        0 => "just now".into(),
        minutes @ 1..60 => format!("{minutes} min ago"),
        minutes @ 60..2880 => format!("{} h ago", minutes / 60),
        minutes => format!("{} d ago", minutes / 1440),
    }
}
