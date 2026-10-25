//! Quick edits in a code tab: the tab's file as plain text to type into,
//! for a fix too small to open an editor for. It is a text field over many
//! lines (selection, the clipboard, undo, the platform's input methods),
//! not an IDE: no completion, no colouring while editing. The file is read
//! and saved on the background executor; saving refuses to write over a
//! file an agent changed meanwhile (`file`), and a change on disk reloads
//! the text while it holds no edits of its own.
//!
//! The editor is an entity of its own, as the platform input API wants one
//! to call back; the code tab holds it while editing and draws it in place
//! of the read-only view (see `code_view`).

mod buffer;
mod display;
pub mod error;
mod file;
mod input;
mod keys;
mod render;

#[cfg(test)]
mod tests;

pub use error::Error;

use crate::config::{FontConfig, Theme};
use buffer::Buffer;
use display::Shown;
use file::Loaded;
use gpui::{
    App, Bounds, Context, EventEmitter, FocusHandle, Focusable, Pixels, ShapedLine, Task,
    UniformListScrollHandle,
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};

/// What the editor tells the tab holding it.
#[derive(Debug)]
pub(crate) enum Event {
    /// The text changed, or was saved: the tab redraws its header.
    Changed,
    Saved,
    /// Saving failed; the reason is the editor's `failure`.
    Failed,
    /// Escape: the user is done editing.
    Done,
}

/// Where the editor stands with the file on disk.
#[derive(Debug, Default)]
pub(crate) enum Disk {
    #[default]
    Loading,
    Ready,
    Saving,
    /// The last save or load failed.
    Failed(Error),
}

/// One line as last drawn: where, and its laid-out text, for the mouse and
/// the input method's candidate window.
struct Painted {
    bounds: Bounds<Pixels>,
    shown: Shown,
    layout: ShapedLine,
}

pub(crate) struct CodeEditor {
    pub(crate) focus: FocusHandle,
    path: PathBuf,
    buffer: Buffer,
    /// The file as last read or saved: what a save expects to find on disk.
    base: Option<Loaded>,
    pub(crate) disk: Disk,
    font: FontConfig,
    theme: Theme,
    line_height: f32,
    scroll: UniformListScrollHandle,
    painted: HashMap<usize, Painted>,
    selecting: bool,
    /// Indent Tab inserts: a tab in a file indented with tabs.
    indent: &'static str,
    /// The 1-based line to put the cursor on once the file is read.
    target: Option<u32>,
    task: Option<Task<()>>,
}

impl EventEmitter<Event> for CodeEditor {}

impl Focusable for CodeEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl CodeEditor {
    /// An editor on `path`, starting to read it.
    pub(crate) fn new(
        path: PathBuf,
        font: FontConfig,
        theme: Theme,
        line_height: f32,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut editor = Self {
            focus: cx.focus_handle(),
            path,
            buffer: Buffer::default(),
            base: None,
            disk: Disk::Loading,
            font,
            theme,
            line_height,
            scroll: UniformListScrollHandle::new(),
            painted: HashMap::new(),
            selecting: false,
            indent: "    ",
            target: None,
            task: None,
        };
        editor.reload(cx);
        editor
    }

    /// Whether the text differs from the file as last read or saved.
    pub(crate) fn dirty(&self) -> bool {
        self.base
            .as_ref()
            .is_some_and(|base| base.text != self.buffer.text())
    }

    pub(crate) fn busy(&self) -> bool {
        self.task.is_some()
    }

    /// Puts the cursor on 1-based `line`, as the read-only view had it.
    pub(crate) fn go_to_line(&mut self, line: u32) {
        if self.base.is_none() {
            self.target = Some(line);
            return;
        }
        let line = usize::try_from(line).unwrap_or(1).saturating_sub(1);
        let start = self.buffer.line_range(line).start;
        self.buffer.select_to(start, false);
        self.scroll
            .scroll_to_item_strict(line, gpui::ScrollStrategy::Center);
    }

    /// Reads the file again, dropping any edits.
    pub(crate) fn reload(&mut self, cx: &mut Context<Self>) {
        let path = self.path.clone();
        let read = cx
            .background_executor()
            .spawn(async move { file::load(&path) });
        self.task = Some(cx.spawn(async move |this, cx| {
            let loaded = read.await;
            let _ = this.update(cx, |this, cx| {
                this.task = None;
                match loaded {
                    Ok(loaded) => this.loaded(loaded),
                    Err(error) => this.disk = Disk::Failed(error),
                }
                cx.emit(Event::Changed);
                cx.notify();
            });
        }));
    }

    fn loaded(&mut self, loaded: Loaded) {
        // Our own save, seen again on disk: keep the cursor and history.
        if self.base.as_ref() == Some(&loaded) && !self.dirty() {
            self.disk = Disk::Ready;
            return;
        }
        let tabs = loaded.text.lines().any(|line| line.starts_with('\t'));
        self.indent = if tabs { "\t" } else { "    " };
        self.buffer.reset(loaded.text.clone());
        self.base = Some(loaded);
        self.disk = Disk::Ready;
        if let Some(line) = self.target.take() {
            self.go_to_line(line);
        }
    }

    /// The file changed on disk: reloads it when the text holds no edits,
    /// or else leaves the next save to find the change.
    pub(crate) fn disk_changed(&mut self, cx: &mut Context<Self>) {
        if !self.dirty() && !self.busy() {
            self.reload(cx);
        }
    }

    /// Writes the text to the file; `force` writes over a change on disk.
    pub(crate) fn save(&mut self, force: bool, cx: &mut Context<Self>) {
        let Some(base) = self.base.clone().filter(|_| !self.busy()) else {
            return;
        };
        let path = self.path.clone();
        let text: Arc<str> = self.buffer.text().into();
        let write = cx
            .background_executor()
            .spawn(async move { file::save(&path, &base, &text, force) });
        self.disk = Disk::Saving;
        self.task = Some(cx.spawn(async move |this, cx| {
            let saved = write.await;
            let _ = this.update(cx, |this, cx| {
                this.task = None;
                match saved {
                    Ok(saved) => {
                        this.base = Some(saved);
                        this.disk = Disk::Ready;
                        cx.emit(Event::Saved);
                    }
                    Err(error) => {
                        this.disk = Disk::Failed(error);
                        cx.emit(Event::Failed);
                    }
                }
                cx.emit(Event::Changed);
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Whether the last save found the file changed on disk.
    pub(crate) fn conflicted(&self) -> bool {
        matches!(self.disk, Disk::Failed(Error::Changed))
    }

    pub(crate) fn failure(&self) -> Option<&Error> {
        match &self.disk {
            Disk::Failed(error) => Some(error),
            _ => None,
        }
    }

    /// Scrolls the cursor's line into view, by as little as it takes.
    fn reveal_cursor(&self) {
        let line = self.buffer.line_of(self.buffer.cursor);
        self.scroll
            .scroll_to_item(line, gpui::ScrollStrategy::Nearest);
    }

    fn edited(&mut self, changed: bool, cx: &mut Context<Self>) {
        if changed {
            cx.emit(Event::Changed);
        }
        self.reveal_cursor();
        cx.notify();
    }
}
