//! What a code tab shows of its editor: the tab's title marked while edits
//! are unsaved, and the header's word on the file: saving, a failed save,
//! or a file an agent changed under unsaved edits, with the choice to
//! overwrite it or revert to it. Reverting is one more edit, which undo
//! takes back, so neither choice loses text.

use super::CodeView;
use crate::{
    HerdrWindow,
    browser::TabId,
    code_edit::{CodeEditor, Event},
    window::Flash,
};
use gpui::{prelude::*, *};

impl HerdrWindow {
    pub(super) fn code_edit_event(
        &mut self,
        id: TabId,
        editor: &Entity<CodeEditor>,
        event: &Event,
        cx: &mut Context<Self>,
    ) {
        match event {
            Event::Changed => cx.notify(),
            // The outline follows the saved text.
            Event::Saved => self.load_code_view(id, cx),
            Event::Failed => {
                let editor = editor.read(cx);
                // A change on disk is the header's to explain.
                if let Some(failure) = editor.failure().filter(|_| !editor.conflicted()) {
                    let failure = failure.to_string();
                    self.show_flash(Flash::warning(failure), cx);
                }
            }
        }
    }

    pub(crate) fn code_editor(&self, id: TabId) -> Option<&Entity<CodeEditor>> {
        self.code_views.get(&id).map(|view| &view.editor)
    }

    /// Whether the keyboard is in a code tab's editor, whose keys and typed
    /// text are its own.
    pub(crate) fn code_editor_focused(&self, window: &Window) -> bool {
        self.code_views
            .values()
            .any(|view| view.focus.is_focused(window))
    }

    /// Whether tab `id` is a code tab holding edits not yet saved.
    pub(crate) fn unsaved_code_edits(&self, id: TabId, cx: &App) -> bool {
        self.code_editor(id)
            .is_some_and(|editor| editor.read(cx).dirty())
    }

    /// The header's word on the file, and its buttons.
    pub(super) fn code_edit_header(
        &self,
        id: TabId,
        view: &CodeView,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = &self.theme;
        let editor = view.editor.read(cx);
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
        let state = |label: SharedString, color: Rgba| {
            div()
                .debug_selector(|| "code-edit-state".into())
                .flex_none()
                .text_color(color)
                .child(label)
                .into_any_element()
        };
        if editor.conflicted() {
            let on = |save: bool| {
                cx.listener(move |this: &mut Self, _: &ClickEvent, _, cx| {
                    if let Some(editor) = this.code_editor(id).cloned() {
                        editor.update(cx, |editor, cx| {
                            if save {
                                editor.save(true, cx);
                            } else {
                                editor.revert(cx);
                            }
                        });
                    }
                })
            };
            return vec![
                state(
                    "Changed on disk since your edits".into(),
                    crate::menu::danger(theme),
                ),
                button("Overwrite", "code-edit-overwrite")
                    .on_click(on(true))
                    .into_any_element(),
                button("Revert", "code-edit-revert")
                    .on_click(on(false))
                    .into_any_element(),
            ];
        }
        if let Some(failure) = editor.failure() {
            return vec![state(
                failure.to_string().into(),
                crate::menu::danger(theme),
            )];
        }
        if editor.saving() {
            return vec![state("Saving\u{2026}".into(), rgb(theme.muted))];
        }
        Vec::new()
    }
}
