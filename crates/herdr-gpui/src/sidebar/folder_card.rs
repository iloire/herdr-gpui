//! The card a workspace row shows on hover: its name, its branch, and the
//! folder its new tabs start in. That folder follows the workspace's shell,
//! so it can leave the checkout the row names; the card then says so.

use crate::{
    HerdrWindow,
    config::{FontConfig, Theme, corners, mix},
    fonts::StyledFont,
};
use gpui::{prelude::*, *};
use herdr_client::protocol::ClientShellWorkspace;

/// What the card draws, taken from the row when it renders. The path is
/// written with `~` only when the tooltip opens, not on every frame.
#[derive(Clone, Debug)]
pub(super) struct FolderCard {
    name: SharedString,
    branch: Option<SharedString>,
    folder: SharedString,
    /// Whether the folder is on this computer, whose home `~` names.
    local: bool,
    left_checkout: bool,
    colors: Colors,
    mono: FontConfig,
}

#[derive(Clone, Copy, Debug)]
struct Colors {
    foreground: u32,
    surface: u32,
    muted: u32,
    subtext: u32,
    rule: u32,
    warn: u32,
}

impl HerdrWindow {
    /// The card a workspace row shows on hover, if any. A carried row shows
    /// none, and neither does a row whose hover opens the workspace menu,
    /// which already shows the folder.
    pub(super) fn folder_card(
        &self,
        workspace: &ClientShellWorkspace,
        name: &str,
        local: bool,
        carried: bool,
    ) -> Option<FolderCard> {
        if carried || self.config.features.sidebar_hover_menu {
            return None;
        }
        FolderCard::of(workspace, name, local, &self.theme, &self.config.terminal)
    }
}

impl FolderCard {
    /// The card for `workspace`, or none when the daemon reports no folder.
    pub(super) fn of(
        workspace: &ClientShellWorkspace,
        name: &str,
        local: bool,
        theme: &Theme,
        mono: &FontConfig,
    ) -> Option<Self> {
        if workspace.new_workspace_cwd.is_empty() {
            return None;
        }
        Some(Self {
            name: SharedString::from(name.to_owned()),
            branch: workspace
                .branch
                .as_deref()
                .filter(|branch| !branch.trim().is_empty())
                .map(|branch| SharedString::from(branch.to_owned())),
            folder: SharedString::from(workspace.new_workspace_cwd.clone()),
            local,
            left_checkout: crate::worktree::workspace_left_checkout(workspace),
            colors: Colors {
                foreground: theme.foreground,
                surface: theme.surface,
                muted: theme.muted,
                subtext: theme.subtext(),
                rule: theme.active,
                warn: theme.ink(theme.palette[3]),
            },
            mono: mono.clone(),
        })
    }

    /// The row's tooltip: a fresh card each time it opens.
    pub(super) fn tooltip(self) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_, cx| cx.new(|_| self.clone()).into()
    }

    /// The folder as the card writes it.
    fn shown_folder(&self) -> SharedString {
        if self.local {
            crate::folder::home_relative(&self.folder).into()
        } else {
            self.folder.clone()
        }
    }
}

impl Render for FolderCard {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors;
        let size = px(12.);
        let icon = |path: &'static str| {
            svg()
                .path(path)
                .size(px(11.))
                .flex_none()
                .text_color(rgb(colors.muted))
        };
        div()
            .debug_selector(|| "folder-card".into())
            .max_w(px(320.))
            .px(px(8.))
            .py(px(6.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .rounded(px(corners::CONTROL))
            .shadow_md()
            .text_size(size)
            .text_color(rgb(colors.foreground))
            .bg(rgb(colors.surface))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .whitespace_normal()
                    .child(crate::sidebar::label_text(&self.name)),
            )
            .when_some(self.branch.clone(), |card, branch| {
                card.child(
                    div()
                        .debug_selector(|| "folder-card-branch".into())
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .text_color(rgb(colors.subtext))
                        .child(icon("icons/git-branch.svg"))
                        .child(div().min_w_0().truncate().child(branch)),
                )
            })
            .child(div().h(px(1.)).bg(rgb(colors.rule)))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(4.))
                    // The icon sits on the path's first line, not its middle.
                    .child(icon("icons/folder.svg").mt(px(2.)))
                    .child(
                        div()
                            .debug_selector(|| "folder-card-path".into())
                            .min_w_0()
                            .whitespace_normal()
                            .text_font(&self.mono)
                            .text_size(size)
                            .child(self.shown_folder()),
                    ),
            )
            .when(self.left_checkout, |card| {
                card.child(
                    div()
                        .debug_selector(|| "folder-card-left-checkout".into())
                        .mt(px(2.))
                        .px(px(4.))
                        .rounded(px(corners::SMALL))
                        .bg(rgb(mix(colors.surface, colors.warn, 15)))
                        .text_color(rgb(colors.warn))
                        .child("Shell left the checkout"),
                )
            })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
