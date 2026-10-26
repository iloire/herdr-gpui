//! A code tab's editor: the tab's file, coloured by its syntax, to type
//! into as in any editor. It is a text field over many lines (selection,
//! the clipboard, undo, the platform's input methods), not an IDE: no
//! completion, no language server.
//!
//! Edits are saved a moment after typing stops, and on Cmd-S, when the tab
//! closes, and when the app quits (`disk`). Saving refuses to write over a
//! file an agent changed meanwhile (`file`); the tab then offers to keep
//! these edits or take the file's, which is one more step undo takes back.
//! A file changed on disk while the editor holds no edits of its own is
//! taken the same way.
//!
//! The editor is an entity of its own, as the platform input API wants one
//! to call back; the code tab draws it beside the file's outline (see
//! `code_view`).

mod buffer;
mod colour;
mod disk;
mod display;
pub mod error;
mod file;
mod input;
mod keys;
mod render;

#[cfg(test)]
mod tests;

pub use error::Error;

use crate::{
    config::{FontConfig, Theme},
    review::highlight::Span,
};
use buffer::Buffer;
use display::Shown;
use file::Loaded;
use gpui::{
    App, Bounds, Context, EventEmitter, FocusHandle, Focusable, Pixels, ShapedLine, Subscription,
    Task, UniformListScrollHandle,
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};

/// What the editor tells the tab holding it.
#[derive(Debug)]
pub(crate) enum Event {
    /// The text, or where it stands with the file, changed: the tab
    /// redraws its header and title.
    Changed,
    /// A save wrote the file.
    Saved,
    /// Reading or saving failed; the reason is the editor's `failure`.
    Failed,
}

/// Where the editor stands with the file on disk.
#[derive(Debug, Default)]
pub(crate) enum Disk {
    #[default]
    Loading,
    Ready,
    Saving,
    /// The last save or read failed.
    Failed(Error),
}

/// One line as last drawn: where, and its laid-out text, for the mouse and
/// the input method's candidate window.
struct Painted {
    bounds: Bounds<Pixels>,
    shown: Shown,
    layout: ShapedLine,
}

/// How the editor is drawn, from the window's settings.
#[derive(Clone, PartialEq)]
pub(crate) struct Look {
    pub(crate) font: FontConfig,
    pub(crate) theme: Theme,
    pub(crate) line_height: f32,
}

pub(crate) struct CodeEditor {
    pub(crate) focus: FocusHandle,
    path: PathBuf,
    buffer: Buffer,
    /// The file as last read or saved: what a save expects to find on disk.
    base: Option<Loaded>,
    disk: Disk,
    look: Look,
    /// Each line's syntax colours, from the text as last coloured.
    spans: Arc<Vec<Vec<Span>>>,
    scroll: UniformListScrollHandle,
    painted: HashMap<usize, Painted>,
    /// The text area as last drawn, which the cursor is kept inside.
    viewport: Option<Bounds<Pixels>>,
    /// Whether the next paint scrolls sideways to the cursor: set by edits
    /// and moves, so scrolling by hand is left alone.
    follow: bool,
    selecting: bool,
    /// Indent Tab inserts: a tab in a file indented with tabs.
    indent: &'static str,
    /// The 1-based line to put the cursor on once the file is read.
    target: Option<u32>,
    /// A read or save under way.
    task: Option<Task<()>>,
    autosave: Option<Task<()>>,
    colouring: Option<Task<()>>,
    _flush: [Subscription; 2],
}

impl EventEmitter<Event> for CodeEditor {}

impl Focusable for CodeEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl CodeEditor {
    /// An editor on `path`, starting to read it.
    pub(crate) fn new(path: PathBuf, look: Look, cx: &mut Context<Self>) -> Self {
        // Whatever is unsaved is written when the tab goes, and before the
        // app quits.
        let flush = [
            cx.on_release(|this, cx| {
                if let Some(write) = this.flush(cx) {
                    write.detach();
                }
            }),
            cx.on_app_quit(|this, cx| {
                let write = this.flush(cx);
                async move {
                    if let Some(write) = write {
                        write.await;
                    }
                }
            }),
        ];
        let mut editor = Self {
            focus: cx.focus_handle(),
            path,
            buffer: Buffer::default(),
            base: None,
            disk: Disk::Loading,
            look,
            spans: Arc::default(),
            scroll: UniformListScrollHandle::new(),
            painted: HashMap::new(),
            viewport: None,
            follow: false,
            selecting: false,
            indent: "    ",
            target: None,
            task: None,
            autosave: None,
            colouring: None,
            _flush: flush,
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

    /// Whether the file has been read.
    #[cfg(test)]
    pub(crate) fn loaded(&self) -> bool {
        self.base.is_some()
    }

    pub(crate) fn saving(&self) -> bool {
        matches!(self.disk, Disk::Saving)
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

    /// How many lines are coloured.
    #[cfg(test)]
    pub(crate) fn coloured_lines(&self) -> usize {
        self.spans.len()
    }

    /// How many lines the text has, not counting an empty last one.
    #[cfg(test)]
    pub(crate) fn line_count(&self) -> usize {
        self.buffer.text().lines().count()
    }

    /// The 1-based line the cursor is on.
    pub(crate) fn cursor_line(&self) -> u32 {
        u32::try_from(self.buffer.line_of(self.buffer.cursor) + 1).unwrap_or(u32::MAX)
    }

    pub(crate) fn set_look(&mut self, look: Look, cx: &mut Context<Self>) {
        if self.look != look {
            self.look = look;
            cx.notify();
        }
    }

    /// Puts the cursor on 1-based `line`, in the middle of the view.
    pub(crate) fn go_to_line(&mut self, line: u32, cx: &mut Context<Self>) {
        // A read under way may bring the line: it goes there once done.
        if matches!(self.disk, Disk::Loading) {
            self.target = Some(line);
            return;
        }
        let line = usize::try_from(line)
            .unwrap_or(usize::MAX)
            .saturating_sub(1)
            .min(self.buffer.line_count() - 1);
        let start = self.buffer.line_range(line).start;
        self.buffer.select_to(start, false);
        self.scroll
            .scroll_to_item_strict(line, gpui::ScrollStrategy::Center);
        self.follow = true;
        cx.notify();
    }

    /// Scrolls the cursor's line into view, by as little as it takes.
    fn reveal_cursor(&mut self) {
        let line = self.buffer.line_of(self.buffer.cursor);
        self.scroll
            .scroll_to_item(line, gpui::ScrollStrategy::Nearest);
        self.follow = true;
    }

    fn edited(&mut self, changed: bool, cx: &mut Context<Self>) {
        if changed {
            self.text_changed(cx);
        }
        self.reveal_cursor();
        cx.notify();
    }
}
