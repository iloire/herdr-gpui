//! Drawing a checkout's card from the last read, as a folder's Changes
//! lists it: its branch, its uncommitted files (`files`), and its latest
//! commits, with the review of what is picked beside it (`pane`). A
//! checkout's header opens the workspace open in it, if any; a right click
//! on a commit copies its name.

use super::{Rank, Shown, pane::Pick};
use crate::{HerdrWindow, window::Flash};
use gpui::{prelude::*, *};
use herdr_vcs::changes::Commit;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

impl HerdrWindow {
    /// A checkout's card: its branch, uncommitted files, and latest commits.
    pub(crate) fn changes_checkout(
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
            crate::folder::home_relative(&checkout.path)
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
        let mut block = div().flex().flex_col().pb_2().child(header).child(
            div()
                .debug_selector(|| "changes-files".into())
                .flex()
                .flex_col()
                .when(
                    self.change_picked(&Pick::Uncommitted(checkout.path.clone())),
                    |files| files.bg(rgb(theme.active)),
                )
                .children(self.changes_files(&format!("{repo}-{position}"), shown, cx)),
        );
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
        let key = format!("{repo}-{position}");
        for commit in &checkout.commits {
            block = block.child(self.changes_commit(&key, shown, commit, now, cx));
        }
        block
    }

    fn changes_commit(
        &self,
        key: &str,
        shown: &Shown,
        commit: &Commit,
        now: u64,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = &self.theme;
        let small = px(self.config.ui.size - 1.);
        let elapsed = now.saturating_sub(u64::try_from(commit.time).unwrap_or_default());
        let pick = Pick::Commit {
            checkout: shown.checkout.path.clone(),
            sha: commit.sha.clone(),
        };
        let picked = self.change_picked(&pick);
        let sha = commit.sha.clone();
        div()
            .id(SharedString::from(format!(
                "changes-commit-{key}-{}",
                commit.sha
            )))
            .debug_selector(|| "changes-commit".into())
            .px_3()
            .pl(px(32.))
            .py(px(1.))
            .flex()
            .items_center()
            .gap_2()
            .text_size(small)
            .cursor_pointer()
            .when(picked, |row| row.bg(rgb(theme.active)))
            .hover(|style| style.bg(rgb(theme.active)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.click_change(pick.clone(), None, cx);
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    cx.write_to_clipboard(ClipboardItem::new_string(sha.clone()));
                    this.show_flash(Flash::success("Commit name copied"), cx);
                }),
            )
            .child(
                div()
                    .flex_none()
                    .text_color(rgb(theme.muted))
                    .child(commit.short().to_owned()),
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

/// How long ago, to the minute while recent and to the day once old: a
/// commit may be months old.
pub(crate) fn age(elapsed: Duration) -> String {
    match elapsed.as_secs() / 60 {
        0 => "just now".into(),
        minutes @ 1..60 => format!("{minutes} min ago"),
        minutes @ 60..2880 => format!("{} h ago", minutes / 60),
        minutes => format!("{} d ago", minutes / 1440),
    }
}
