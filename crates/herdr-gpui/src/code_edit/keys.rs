//! The editor's keys: moving and selecting, deleting, the clipboard, undo,
//! and saving, as a text editor on the platform takes them. Printable text
//! never comes through here: it arrives from the platform input API
//! (`input`), so input methods and dead keys compose as they do elsewhere.

use super::{CodeEditor, Event};
use crate::actions;
use gpui::{ClipboardItem, Context, KeyDownEvent, Window};

/// Lines Page Up and Page Down move.
const PAGE: isize = 30;

impl CodeEditor {
    pub(super) fn key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // The input method owns the keys while it composes.
        if self.buffer.marked.is_some() || self.base.is_none() {
            return;
        }
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let shift = modifiers.shift;
        let secondary = modifiers.secondary() && !modifiers.alt && !modifiers.function;
        let plain =
            !modifiers.platform && !modifiers.control && !modifiers.alt && !modifiers.function;
        let cursor = self.buffer.cursor;
        let changed = if secondary {
            match key {
                "s" if !shift => {
                    self.save(false, cx);
                    false
                }
                "z" if shift => self.buffer.redo(),
                "z" => self.buffer.undo(),
                "y" if !cfg!(target_os = "macos") => self.buffer.redo(),
                "a" if !shift => {
                    self.buffer.select_all();
                    false
                }
                "c" | "x" if !shift => self.copy(key == "x", cx),
                "v" if !shift => self.paste(cx),
                // As in every macOS text view: Cmd-arrows go to the ends of
                // the line and of the file.
                "left" if cfg!(target_os = "macos") => {
                    self.buffer.select_to(self.buffer.line_start(cursor), shift);
                    false
                }
                "right" if cfg!(target_os = "macos") => {
                    self.buffer.select_to(self.buffer.line_end(cursor), shift);
                    false
                }
                "up" | "home" => {
                    self.buffer.select_to(0, shift);
                    false
                }
                "down" | "end" => {
                    self.buffer.select_to(self.buffer.text().len(), shift);
                    false
                }
                "backspace" if cfg!(target_os = "macos") => {
                    self.buffer.delete_to(self.buffer.line_start(cursor))
                }
                _ => return,
            }
        } else if actions::word_modifiers(modifiers)
            && matches!(key, "left" | "right" | "backspace" | "delete")
        {
            let text = self.buffer.text();
            let to = if matches!(key, "left" | "backspace") {
                crate::words::previous_start(text, cursor)
            } else {
                crate::words::next_end(text, cursor)
            };
            if matches!(key, "left" | "right") {
                self.buffer.select_to(to, shift);
                false
            } else {
                self.buffer.delete_to(to)
            }
        } else if plain {
            let selection = self.buffer.selection();
            match key {
                "left" if !shift && !selection.is_empty() => {
                    self.buffer.select_to(selection.start, false);
                    false
                }
                "right" if !shift && !selection.is_empty() => {
                    self.buffer.select_to(selection.end, false);
                    false
                }
                "left" => {
                    self.buffer.select_to(self.buffer.previous(cursor), shift);
                    false
                }
                "right" => {
                    self.buffer.select_to(self.buffer.next(cursor), shift);
                    false
                }
                "up" => {
                    self.buffer.move_vertical(-1, shift);
                    false
                }
                "down" => {
                    self.buffer.move_vertical(1, shift);
                    false
                }
                "pageup" => {
                    self.buffer.move_vertical(-PAGE, shift);
                    false
                }
                "pagedown" => {
                    self.buffer.move_vertical(PAGE, shift);
                    false
                }
                "home" => {
                    self.buffer.select_to(self.buffer.line_start(cursor), shift);
                    false
                }
                "end" => {
                    self.buffer.select_to(self.buffer.line_end(cursor), shift);
                    false
                }
                "backspace" if !shift => self.buffer.delete_to(self.buffer.previous(cursor)),
                "delete" if !shift => self.buffer.delete_to(self.buffer.next(cursor)),
                "enter" if !shift => self.buffer.newline(),
                "tab" if !shift => self.buffer.insert(self.indent),
                "escape" if !shift => {
                    cx.emit(Event::Done);
                    cx.stop_propagation();
                    return;
                }
                _ => return,
            }
        } else {
            return;
        };
        cx.stop_propagation();
        window.prevent_default();
        self.edited(changed, cx);
    }

    /// Copies the selection, cutting it with `cut`; whether the text changed.
    fn copy(&mut self, cut: bool, cx: &mut Context<Self>) -> bool {
        let selected = self.buffer.selected();
        if selected.is_empty() {
            return false;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(selected.to_owned()));
        cut && self.buffer.insert("")
    }

    fn paste(&mut self, cx: &mut Context<Self>) -> bool {
        match cx.read_from_clipboard().and_then(|item| item.text()) {
            Some(text) => self.buffer.insert(&text),
            None => false,
        }
    }
}
