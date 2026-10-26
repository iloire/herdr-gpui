//! A folder row's context menu: what can be done with the folder itself,
//! whichever host it is on. It holds a copy of the folder's path, so a
//! snapshot that drops the workspace while the menu is open changes nothing.

use super::model::Card;
use crate::{HerdrWindow, menu::Page, window::Flash};
use gpui::{prelude::*, *};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    CopyPath,
    /// Only this computer's folders open here.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    OpenFolder,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::CopyPath => "Copy Path",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Self::OpenFolder => "Open Folder",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::CopyPath => "icons/copy.svg",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Self::OpenFolder => "icons/external.svg",
        }
    }
}

pub(crate) struct FolderMenu {
    /// The folder's full path, as its host names it.
    folder: String,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    local: bool,
    selected: Option<usize>,
}

impl FolderMenu {
    pub(super) fn new(card: &Card) -> Self {
        Self {
            folder: card.folder.clone(),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            local: card.host.is_none(),
            selected: None,
        }
    }

    pub(super) fn actions(&self) -> Vec<Action> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        if self.local {
            return vec![Action::CopyPath, Action::OpenFolder];
        }
        vec![Action::CopyPath]
    }
}

impl HerdrWindow {
    pub(super) fn open_folder_menu(
        &mut self,
        card: &Card,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if card.folder.is_empty() || !self.open_menu(window, cx) {
            return;
        }
        self.menu.anchor = anchor;
        self.menu.page = Some(Page::Folder);
        self.menu.folder = Some(FolderMenu::new(card));
        self.menu.opening_right_click = true;
    }

    fn activate_folder_menu(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.menu.folder.take() else {
            return;
        };
        self.dismiss_menu(window, cx);
        match action {
            Action::CopyPath => {
                cx.write_to_clipboard(ClipboardItem::new_string(menu.folder));
                self.show_flash(Flash::success("Path copied"), cx);
            }
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Action::OpenFolder => crate::folder::open_path(false, menu.folder.into(), cx),
        }
    }

    pub(crate) fn folder_menu_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = &mut self.menu.folder else {
            return;
        };
        cx.stop_propagation();
        window.prevent_default();
        match event.keystroke.key.as_str() {
            "escape" => self.dismiss_menu(window, cx),
            key @ ("up" | "down") => {
                let count = menu.actions().len();
                menu.selected = Some(match (menu.selected, key) {
                    (None, "up") => count - 1,
                    (None, _) => 0,
                    (Some(i), "up") => (i + count - 1) % count,
                    (Some(i), _) => (i + 1) % count,
                });
                cx.notify();
            }
            "enter" => {
                if let Some(action) = menu
                    .selected
                    .and_then(|index| menu.actions().get(index).copied())
                {
                    self.activate_folder_menu(action, window, cx);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn render_folder_menu(&self, cx: &mut Context<Self>) -> Div {
        let Some(menu) = &self.menu.folder else {
            return div();
        };
        let theme = &self.theme;
        div()
            .flex()
            .flex_col()
            .children(
                menu.actions()
                    .into_iter()
                    .enumerate()
                    .map(|(index, action)| {
                        div()
                            .id(("folder-menu-action", index))
                            .debug_selector(move || format!("folder-menu-{index}"))
                            .min_h(px(self.config.ui.line_height() + 12.))
                            .px(px(8.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .cursor_pointer()
                            .when(menu.selected == Some(index), |row| {
                                row.bg(rgb(theme.active))
                            })
                            .hover(|row| row.bg(rgb(theme.active)))
                            .child(
                                svg()
                                    .flex_none()
                                    .size(px(13.))
                                    .path(action.icon())
                                    .text_color(rgb(theme.muted)),
                            )
                            .child(div().min_w_0().truncate().child(action.label()))
                            .on_hover(cx.listener(move |this, hovered, _, cx| {
                                if *hovered && let Some(menu) = &mut this.menu.folder {
                                    menu.selected = Some(index);
                                    cx.notify();
                                }
                            }))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.activate_folder_menu(action, window, cx)
                            }))
                    }),
            )
    }
}
