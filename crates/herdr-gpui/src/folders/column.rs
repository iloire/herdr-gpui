//! Folders as a column beside the terminals: with a folder's Terminals or
//! Files picked, the folder list stands where the Workspaces sidebar would,
//! with that folder's file tree beside it for Files, and the window draws
//! its own terminal area across the rest. The sidebar module sizes and
//! places the column (`sidebar::activity`); this draws it.

use super::Part;
use crate::{HerdrWindow, fonts::StyledFont, panel_resize::PanelDrag};
use gpui::{prelude::*, *};

impl HerdrWindow {
    /// Whether the folder shown is at a part beside the terminals.
    pub(crate) fn folders_beside_terminals(&self) -> bool {
        self.folders.part.beside_terminals()
    }

    /// Whether the folder shown is at its Files, whose tree and file tabs
    /// are the Files view's.
    pub(crate) fn folders_files(&self) -> bool {
        self.folders.part == Part::Files
    }

    /// Whether the window shows a folder's Files, which lists and shows
    /// only files: its terminals wait under Terminals.
    pub(crate) fn files_only(&self) -> bool {
        self.folders_column() && self.folders_files()
    }

    /// The column's width: the list, and for Files the tree beside it,
    /// each as dragged.
    pub(crate) fn folders_column_width(&self) -> f32 {
        let list = self.folders.list_width.width(self.viewport_width);
        let tree = self.files_tree_width.width(self.viewport_width);
        list + if self.folders_files() { tree } else { 0. }
    }

    /// The column: the folder list, then for Files the tree, its rows
    /// `row_height` tall.
    pub(crate) fn render_folders_column(&self, row_height: f32, cx: &mut Context<Self>) -> Div {
        let look = self.overview_look(cx);
        let theme = look.theme;
        let cards = self.folder_cards(cx);
        let listed = self.listed_folders(&cards);
        let selected = self.selected_folder(&listed);
        let list = div()
            .flex()
            .h_full()
            .min_h_0()
            .text_color(rgb(theme.foreground))
            .text_font(look.ui)
            .text_size(px(look.ui.size))
            .child(self.render_folder_list(&look, &cards, &listed, selected, cx));
        let files = self.folders_files();
        div()
            .debug_selector(|| "folders-column".into())
            .flex()
            .flex_none()
            .h_full()
            .min_h_0()
            .child(list)
            .when(files, |column| {
                let tree = self.files_tree_width.width(self.viewport_width);
                column.child(self.resizable_panel(
                    self.render_files(tree, row_height, cx),
                    "folders-tree-resize",
                    PanelDrag::FilesTree,
                    None,
                    cx,
                ))
            })
    }
}
