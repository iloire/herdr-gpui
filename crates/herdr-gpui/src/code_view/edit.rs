//! A code tab turned editable: Edit (or `i`) swaps the read-only view for a
//! `code_edit::CodeEditor` on the same file, at the line in view, and Done
//! (or Escape) swaps it back once the edits are saved or discarded. The
//! header says what the editor holds: unsaved edits, a save under way, or a
//! file changed on disk under unsaved edits, with the choice to keep either.

use super::CodeView;
use crate::{
    HerdrWindow,
    browser::{Location, Store, TabId},
    code_edit::{CodeEditor, Event},
    window::Flash,
};
use gpui::{prelude::*, *};

/// The editor a code tab holds while editing.
pub(crate) struct Editing {
    pub(super) editor: Entity<CodeEditor>,
    _events: Subscription,
}

impl HerdrWindow {
    /// Makes tab `id` editable, at the line in view.
    pub(crate) fn edit_code_view(
        &mut self,
        id: TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Location::Code { file }) = cx
            .try_global::<Store>()
            .and_then(|store| store.get(id))
            .and_then(|tab| tab.location.clone())
        else {
            return;
        };
        let line_height = self.code_line_height();
        let (font, theme) = (self.config.terminal.clone(), self.theme.clone());
        let Some(view) = self.code_views.get_mut(&id) else {
            return;
        };
        if let Some(editing) = &view.editing {
            let focus = editing.editor.read(cx).focus.clone();
            window.focus(&focus, cx);
            return;
        }
        let line = view.editor_line();
        let editor = cx.new(|cx| {
            let mut editor = CodeEditor::new(file.path.into(), font, theme, line_height, cx);
            editor.go_to_line(line);
            editor
        });
        let events = cx.subscribe_in(
            &editor,
            window,
            move |this, _, event: &Event, window, cx| {
                this.code_edit_event(id, event, window, cx);
            },
        );
        let focus = editor.read(cx).focus.clone();
        view.editing = Some(Editing {
            editor,
            _events: events,
        });
        window.focus(&focus, cx);
        cx.notify();
    }

    fn code_edit_event(
        &mut self,
        id: TabId,
        event: &Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            Event::Changed => cx.notify(),
            Event::Saved => {
                self.show_flash(Flash::success("Saved"), cx);
                // The read-only view shows the saved text once editing ends.
                self.load_code_view(id, cx);
            }
            Event::Failed => {
                let failure = self
                    .code_editor(id)
                    .and_then(|editor| editor.read(cx).failure().map(ToString::to_string));
                if let Some(failure) = failure {
                    self.show_flash(Flash::warning(failure), cx);
                }
            }
            Event::Done => self.finish_code_edit(id, false, window, cx),
        }
    }

    pub(crate) fn code_editor(&self, id: TabId) -> Option<&Entity<CodeEditor>> {
        self.code_views
            .get(&id)
            .and_then(|view| view.editing.as_ref())
            .map(|editing| &editing.editor)
    }

    /// Leaves editing: at once when nothing is unsaved, or with `discard`;
    /// otherwise the user is told to save or discard first.
    pub(crate) fn finish_code_edit(
        &mut self,
        id: TabId,
        discard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = self.code_editor(id).cloned() else {
            return;
        };
        if editor.read(cx).dirty() && !discard {
            self.show_flash(Flash::warning("Unsaved edits: save them, or Discard"), cx);
            return;
        }
        let Some(view) = self.code_views.get_mut(&id) else {
            return;
        };
        view.editing = None;
        let focus = view.focus.clone();
        self.load_code_view(id, cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    /// Whether the keyboard is in a code tab's editor, whose keys and typed
    /// text are its own.
    pub(crate) fn code_editor_focused(&self, window: &Window, cx: &App) -> bool {
        self.code_views
            .values()
            .filter_map(|view| view.editing.as_ref())
            .any(|editing| editing.editor.read(cx).focus.is_focused(window))
    }

    /// The editing part of a code tab's header: what the editor holds, and
    /// its buttons.
    pub(super) fn code_edit_header(
        &self,
        id: TabId,
        view: &CodeView,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = &self.theme;
        let Some(editing) = &view.editing else {
            return Vec::new();
        };
        let editor = editing.editor.read(cx);
        let button = |label: &'static str, selector: &'static str| {
            div()
                .id(selector)
                .debug_selector(move || selector.into())
                .flex_none()
                .px_2()
                .py_1()
                .rounded(px(crate::config::corners::CONTROL))
                .cursor_pointer()
                .hover(|button| button.bg(rgb(theme.active)))
                .child(label)
        };
        let state = if editor.conflicted() {
            Some(("Changed on disk", crate::menu::danger(theme)))
        } else if editor.busy() {
            Some(("Saving\u{2026}", rgb(theme.muted)))
        } else if editor.dirty() {
            Some(("\u{25cf} Edited", rgb(theme.ink(theme.palette[3]))))
        } else {
            None
        };
        let mut items: Vec<AnyElement> = state
            .map(|(label, color)| {
                div()
                    .debug_selector(|| "code-edit-state".into())
                    .flex_none()
                    .text_color(color)
                    .child(label)
                    .into_any_element()
            })
            .into_iter()
            .collect();
        if editor.conflicted() {
            items.push(
                button("Keep mine", "code-edit-overwrite")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(editor) = this.code_editor(id).cloned() {
                            editor.update(cx, |editor, cx| editor.save(true, cx));
                        }
                    }))
                    .into_any_element(),
            );
            items.push(
                button("Take theirs", "code-edit-reload")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(editor) = this.code_editor(id).cloned() {
                            editor.update(cx, |editor, cx| editor.reload(cx));
                        }
                    }))
                    .into_any_element(),
            );
        } else {
            items.push(
                button("Save", "code-edit-save")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(editor) = this.code_editor(id).cloned() {
                            editor.update(cx, |editor, cx| editor.save(false, cx));
                        }
                    }))
                    .into_any_element(),
            );
        }
        let dirty = editor.dirty();
        items.push(
            button(if dirty { "Discard" } else { "Done" }, "code-edit-done")
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.finish_code_edit(id, true, window, cx);
                }))
                .into_any_element(),
        );
        items
    }
}
