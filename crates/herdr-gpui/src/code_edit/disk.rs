//! The editor and the file: reading it, saving a moment after typing
//! stops, taking the file's version when it changes on disk, colouring the
//! text, and the last save when the tab closes or the app quits. All file
//! and colouring work runs on the background executor.

use super::{CodeEditor, Disk, Error, Event, file};
use crate::review::highlight;
use gpui::{App, AsyncApp, Context, Task, WeakEntity};
use std::{sync::Arc, time::Duration};

/// How long after the last keystroke edits are saved.
const AUTOSAVE: Duration = Duration::from_secs(1);
/// How long after the last keystroke the text is coloured again.
const RECOLOUR: Duration = Duration::from_millis(150);
/// Lines coloured at most; the rest of a longer file draws plain.
const MAX_COLOURED_LINES: usize = 20_000;

impl CodeEditor {
    /// Reads the file and takes its text: in full the first time, and after
    /// that as one step undo takes back.
    pub(crate) fn reload(&mut self, cx: &mut Context<Self>) {
        let path = self.path.clone();
        let read = cx
            .background_executor()
            .spawn(async move { file::load(&path) });
        self.disk = Disk::Loading;
        self.task = Some(cx.spawn(async move |this, cx| {
            let loaded = read.await;
            let _ = this.update(cx, |this, cx| {
                this.task = None;
                match loaded {
                    Ok(loaded) => this.take(loaded, cx),
                    Err(error) => {
                        this.disk = Disk::Failed(error);
                        cx.emit(Event::Failed);
                    }
                }
                cx.emit(Event::Changed);
                cx.notify();
            });
        }));
    }

    fn take(&mut self, loaded: file::Loaded, cx: &mut Context<Self>) {
        self.disk = Disk::Ready;
        if self.base.is_none() {
            let tabs = loaded.text.lines().any(|line| line.starts_with('\t'));
            self.indent = if tabs { "\t" } else { "    " };
            self.buffer.reset(loaded.text.clone());
            self.base = Some(loaded);
            self.recolour(Duration::ZERO, cx);
        } else {
            let changed = self.buffer.replace_all(&loaded.text);
            self.base = Some(loaded);
            self.autosave = None;
            if changed {
                self.recolour(RECOLOUR, cx);
            }
        }
        if let Some(line) = self.target.take() {
            self.go_to_line(line, cx);
        }
    }

    /// The file changed on disk: taken at once while the text holds no
    /// edits; otherwise the next save finds the change and asks.
    pub(crate) fn disk_changed(&mut self, cx: &mut Context<Self>) {
        if !self.dirty() && self.task.is_none() {
            self.reload(cx);
        }
    }

    /// Drops these edits for the file as it is on disk, as one step undo
    /// takes back.
    pub(crate) fn revert(&mut self, cx: &mut Context<Self>) {
        if self.task.is_none() {
            self.reload(cx);
        }
    }

    /// Writes the text to the file; `force` writes over a change on disk.
    pub(crate) fn save(&mut self, force: bool, cx: &mut Context<Self>) {
        let Some(base) = self.base.clone().filter(|_| self.task.is_none()) else {
            return;
        };
        self.autosave = None;
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
                        // Typed while saving: saved in turn.
                        if this.dirty() {
                            this.schedule_autosave(cx);
                        }
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
        cx.emit(Event::Changed);
        cx.notify();
    }

    /// After an edit: saved and coloured again once typing pauses.
    pub(super) fn text_changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(Event::Changed);
        self.schedule_autosave(cx);
        self.recolour(RECOLOUR, cx);
    }

    fn schedule_autosave(&mut self, cx: &mut Context<Self>) {
        // A file changed on disk waits for the user's choice.
        if self.conflicted() {
            return;
        }
        self.autosave = Some(
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                cx.background_executor().timer(AUTOSAVE).await;
                let _ = this.update(cx, |this, cx| {
                    this.autosave = None;
                    if this.dirty() && !this.conflicted() {
                        this.save(false, cx);
                    }
                });
            }),
        );
    }

    /// Colours the text once `after` has passed without another edit.
    fn recolour(&mut self, after: Duration, cx: &mut Context<Self>) {
        let name = self
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.colouring = Some(
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                cx.background_executor().timer(after).await;
                let Ok(text) = this.update(cx, |this, _| Arc::<str>::from(this.buffer.text()))
                else {
                    return;
                };
                let spans = cx
                    .background_executor()
                    .spawn(async move { highlight::colour_file(&name, &text, MAX_COLOURED_LINES) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    this.spans = Arc::new(spans);
                    cx.notify();
                });
            }),
        );
    }

    /// The write of whatever is unsaved, as the editor goes away. Nothing
    /// when the file changed on disk: that waits for the user's choice,
    /// which they can no longer make, so the edits are reported lost.
    pub(super) fn flush(&mut self, cx: &mut App) -> Option<Task<()>> {
        if !self.dirty() {
            return None;
        }
        let path = self.path.clone();
        if self.conflicted() {
            tracing::warn!(path = %path.display(), "unsaved edits dropped: the file changed on disk");
            return None;
        }
        let base = self.base.clone()?;
        let text = self.buffer.text().to_owned();
        Some(cx.background_executor().spawn(async move {
            if let Err(error) = file::save(&path, &base, &text, false) {
                let error: &Error = &error;
                tracing::warn!(path = %path.display(), %error, "could not save edits on close");
            }
        }))
    }
}
