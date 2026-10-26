//! A Files tree row's context menu: what suits the entry, a file or a
//! folder, and for a file its kind. It holds a copy of the checkout's root
//! and the entry's path, so a read that changes the tree while the menu is
//! open changes nothing.

use super::Opening;
use crate::{HerdrWindow, menu::Page, window::Flash};
use gpui::{prelude::*, *};
use herdr_client::Method;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(super) enum Action {
    /// In a code tab, or a markdown file rendered.
    Open,
    /// A markdown file's source, in a code tab.
    OpenSource,
    /// In the terminal editor beside the focused pane.
    OpenInEditor,
    /// A Herdr folder of its own, with a shell there.
    NewFolder,
    /// A file in its default app, a folder in the file manager.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    OpenDefault,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Reveal,
    CopyPath,
    CopyRelativePath,
}

pub(crate) struct FilesMenu {
    root: PathBuf,
    /// The entry relative to `root`, with `/` between its parts.
    path: String,
    folder: bool,
    selected: Option<usize>,
}

impl FilesMenu {
    pub(super) fn new(root: &Path, path: &str, folder: bool) -> Self {
        Self {
            root: root.to_path_buf(),
            path: path.to_owned(),
            folder,
            selected: None,
        }
    }

    fn full(&self) -> PathBuf {
        self.root.join(&self.path)
    }

    pub(super) fn actions(&self) -> Vec<Action> {
        let mut actions = if self.folder {
            vec![Action::NewFolder]
        } else {
            let mut opens = vec![Action::Open];
            if crate::browser::renders_markdown(&self.full()) {
                opens.push(Action::OpenSource);
            }
            opens.push(Action::OpenInEditor);
            opens
        };
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            actions.push(Action::OpenDefault);
            if !self.folder {
                actions.push(Action::Reveal);
            }
        }
        actions.extend([Action::CopyPath, Action::CopyRelativePath]);
        actions
    }

    pub(super) fn label(&self, action: Action) -> &'static str {
        match action {
            Action::Open if crate::browser::renders_markdown(&self.full()) => "Open Rendered",
            Action::Open => "Open",
            Action::OpenSource => "Open as Source",
            Action::OpenInEditor => "Open in Editor",
            Action::NewFolder => "Open as New Folder",
            #[cfg(target_os = "macos")]
            Action::OpenDefault if self.folder => "Open in Finder",
            #[cfg(target_os = "linux")]
            Action::OpenDefault if self.folder => "Open in File Manager",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Action::OpenDefault if self.is_web_page() => "Open in Browser",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Action::OpenDefault => "Open with Default App",
            #[cfg(target_os = "macos")]
            Action::Reveal => "Reveal in Finder",
            #[cfg(target_os = "linux")]
            Action::Reveal => "Show in File Manager",
            Action::CopyPath => "Copy Path",
            Action::CopyRelativePath => "Copy Relative Path",
        }
    }

    fn icon(&self, action: Action) -> &'static str {
        match action {
            Action::Open => "icons/file.svg",
            Action::OpenSource => "icons/code.svg",
            Action::OpenInEditor => "icons/terminal.svg",
            Action::NewFolder => "icons/plus.svg",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Action::OpenDefault if self.is_web_page() => "icons/globe.svg",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Action::OpenDefault => "icons/external.svg",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Action::Reveal => "icons/folder.svg",
            Action::CopyPath | Action::CopyRelativePath => "icons/copy.svg",
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn is_web_page(&self) -> bool {
        let extension = Path::new(&self.path)
            .extension()
            .and_then(|extension| extension.to_str());
        !self.folder
            && extension.is_some_and(|extension| {
                extension.eq_ignore_ascii_case("html") || extension.eq_ignore_ascii_case("htm")
            })
    }
}

impl HerdrWindow {
    /// Opens the menu of `path`, relative to the shown checkout, at `anchor`.
    pub(super) fn open_files_menu(
        &mut self,
        path: &str,
        folder: bool,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(root) = self.files.root().map(Path::to_path_buf) else {
            return;
        };
        if !self.open_menu(window, cx) {
            return;
        }
        self.menu.anchor = anchor;
        self.menu.page = Some(Page::Files);
        self.menu.files = Some(FilesMenu::new(&root, path, folder));
        self.menu.opening_right_click = true;
    }

    fn activate_files_menu(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.files.take() else {
            return;
        };
        self.dismiss_menu(window, cx);
        match action {
            Action::Open => self.open_from_files_menu(&menu, Opening::Tab, window, cx),
            Action::OpenSource => self.open_from_files_menu(&menu, Opening::Source, window, cx),
            Action::OpenInEditor => self.open_from_files_menu(&menu, Opening::Editor, window, cx),
            Action::NewFolder => self.open_files_folder(&menu, cx),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Action::OpenDefault => crate::folder::open_path(false, menu.full(), cx),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Action::Reveal => crate::folder::open_path(true, menu.full(), cx),
            Action::CopyPath => {
                let path = menu.full().to_string_lossy().into_owned();
                cx.write_to_clipboard(ClipboardItem::new_string(path));
                self.show_flash(Flash::success("Path copied"), cx);
            }
            Action::CopyRelativePath => {
                cx.write_to_clipboard(ClipboardItem::new_string(menu.path));
                self.show_flash(Flash::success("Relative path copied"), cx);
            }
        }
    }

    fn open_from_files_menu(
        &mut self,
        menu: &FilesMenu,
        opening: Opening,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // The tree may have turned to another checkout meanwhile.
        if self.files.root() == Some(menu.root.as_path()) {
            self.open_files_entry(&menu.path, opening, window, cx);
        }
    }

    /// A Herdr folder in `menu`'s folder, on this computer's Herdr, which
    /// the tree reads. The repository's own scripts are not trusted by it.
    fn open_files_folder(&mut self, menu: &FilesMenu, cx: &mut Context<Self>) {
        let label = Path::new(&menu.path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        let params = serde_json::json!({
            "cwd": menu.full(),
            "label": label,
            "focus": true,
            "trust_repository": false,
        });
        if !self.request_focus_change("Open as new folder", None, |handle, boot| {
            handle.request(boot, Method::WorkspaceCreate, params)
        }) && self.local_error.is_none()
        {
            self.local_error = Some("Open as new folder: not connected to Herdr".into());
        }
        cx.notify();
    }

    pub(crate) fn files_menu_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = &mut self.menu.files else {
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
                    self.activate_files_menu(action, window, cx);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn render_files_menu(&self, cx: &mut Context<Self>) -> Div {
        let Some(menu) = &self.menu.files else {
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
                            .id(("files-menu-action", index))
                            .debug_selector(move || format!("files-menu-{index}"))
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
                                    .path(menu.icon(action))
                                    .text_color(rgb(theme.muted)),
                            )
                            .child(div().min_w_0().truncate().child(menu.label(action)))
                            .on_hover(cx.listener(move |this, hovered, _, cx| {
                                if *hovered && let Some(menu) = &mut this.menu.files {
                                    menu.selected = Some(index);
                                    cx.notify();
                                }
                            }))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.activate_files_menu(action, window, cx)
                            }))
                    }),
            )
    }
}
