//! The workspace menu header's folder line: the folder new tabs start in,
//! with buttons to copy it and to open it in the file manager. It is read
//! from the live snapshot, so a shell that moves while the menu is open
//! moves the line too.

use super::WorkspaceTarget;
use crate::{HerdrWindow, fonts::StyledFont};
use gpui::{prelude::*, *};

impl HerdrWindow {
    pub(in crate::menu) fn render_workspace_folder(
        &self,
        target: &WorkspaceTarget,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let snapshot = self.live.snapshot.as_deref()?;
        if snapshot.boot_id != target.boot_id {
            return None;
        }
        let workspace = snapshot
            .workspaces
            .iter()
            .find(|workspace| workspace.workspace_id == target.id)?;
        let folder = workspace.new_workspace_cwd.clone();
        if folder.is_empty() {
            return None;
        }
        let theme = &self.theme;
        // Only this computer's home is `~`, and only its folders open here.
        let local = self.selected_endpoint == 0;
        let shown = if local {
            crate::folder::home_relative(&folder)
        } else {
            folder.clone()
        };
        let color = if crate::worktree::workspace_left_checkout(workspace) {
            theme.ink(theme.palette[3])
        } else {
            theme.muted
        };
        let copied = folder.clone();
        let line = div()
            .debug_selector(|| "workspace-menu-folder".into())
            .mt(px(2.))
            .flex()
            .items_center()
            .gap(px(4.))
            .child(
                div()
                    .debug_selector(|| "workspace-menu-folder-path".into())
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_font(&self.config.terminal)
                    .text_size(px(self.config.ui.size * 0.9))
                    .text_color(rgb(color))
                    .child(shown),
            )
            .child(
                self.folder_button("copy", "Copy path", "icons/copy.svg")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()));
                        this.dismiss_menu(window, cx);
                    })),
            );
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let line = line.when(local, |line| {
            line.child(
                self.folder_button("open", "Open folder", "icons/external.svg")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        crate::folder::open_path(false, folder.clone().into(), cx);
                        this.dismiss_menu(window, cx);
                    })),
            )
        });
        Some(line)
    }

    fn folder_button(
        &self,
        id: &'static str,
        label: &'static str,
        icon: &'static str,
    ) -> Stateful<Div> {
        let theme = &self.theme;
        let (hover, foreground, surface) = (theme.active, theme.foreground, theme.surface);
        div()
            .id(id)
            .debug_selector(move || format!("workspace-menu-folder-{id}"))
            .role(Role::Button)
            .aria_label(label)
            .flex_none()
            .p(px(2.))
            .rounded(px(crate::config::corners::SMALL))
            .cursor_pointer()
            .hover(move |button| button.bg(rgb(hover)))
            .child(svg().path(icon).size(px(13.)).text_color(rgb(theme.muted)))
            .tooltip(move |_, cx| {
                cx.new(|_| crate::usage::Hint {
                    text: label.into(),
                    foreground,
                    surface,
                })
                .into()
            })
    }
}
