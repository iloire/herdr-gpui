//! Code tabs: a local source file beside the workspace's terminals and
//! pages, open to edit as in any editor (`code_edit`), with its definitions
//! listed as an outline. Open in Editor takes it to the terminal editor at
//! the cursor's line.
//!
//! A tab keeps only the file's path and the line it opened at; the window
//! keeps each tab's view, made when the tab opens or is first seen after a
//! restart: the editor, and the outline, read on the background executor
//! and again after each save. The file is read again when it changes on
//! disk (`browser::disk_watch`), and a markdown file's tab turns to its
//! rendered page and back (`browser::markdown_toggle`).

mod edit;
mod render;

use crate::{
    HerdrWindow,
    browser::{Location, Store, TabId},
    code_edit::{CodeEditor, Event, Look},
    code_index::{self, Kind, Language},
    editor::EditorTarget,
    window::Flash,
};
use gpui::{prelude::*, *};
use serde::{Deserialize, Serialize};
use std::{cell::Cell, path::Path, rc::Rc, sync::Arc};

/// The local file a code tab shows, and the line it opened at. Saved with
/// the tabs, so it is checked again whenever it is read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SavedCodeFile")]
pub(crate) struct CodeFile {
    pub(crate) path: String,
    pub(crate) line: Option<u32>,
}

#[derive(Deserialize)]
struct SavedCodeFile {
    path: String,
    line: Option<u32>,
}

impl TryFrom<SavedCodeFile> for CodeFile {
    type Error = crate::Error;

    fn try_from(saved: SavedCodeFile) -> crate::Result<Self> {
        if saved.path.is_empty()
            || saved.path.len() > 4096
            || saved.path.chars().any(char::is_control)
            || !Path::new(&saved.path).is_absolute()
        {
            return Err(crate::Error::InvalidCodeFile);
        }
        Ok(Self {
            path: saved.path,
            line: saved.line,
        })
    }
}

impl CodeFile {
    /// The file's name, as its tab is titled.
    pub(crate) fn name(&self) -> &str {
        Path::new(&self.path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&self.path)
    }
}

/// One definition in the outline.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    name: String,
    kind: Kind,
    line: u32,
}

/// The file's outline, as last read.
struct Outline {
    entries: Vec<Entry>,
}

impl Outline {
    /// Reads the file at `path` and lists its definitions. Its text is
    /// untrusted: as in a review, tabs become spaces and control and
    /// direction-override characters go before it is scanned, so a name
    /// shown is what was read.
    fn read(path: &Path) -> crate::Result<Self> {
        let raw = code_index::read_source(path).ok_or(crate::Error::CodeFileUnreadable)?;
        let mut source = String::with_capacity(raw.len());
        for line in raw.lines() {
            crate::review::push_clean(&mut source, line);
            source.push('\n');
        }
        let entries = path
            .to_str()
            .and_then(Language::of)
            .map(|language| code_index::scan(language, &source, 5_000))
            .unwrap_or_default()
            .into_iter()
            .map(|found| Entry {
                name: found.name,
                kind: found.kind,
                line: found.line,
            })
            .collect();
        Ok(Self { entries })
    }
}

pub(crate) struct CodeView {
    /// The editor's, which holds the keyboard while the tab is used.
    pub(crate) focus: FocusHandle,
    editor: Entity<CodeEditor>,
    outline: Option<Arc<Outline>>,
    show_outline: bool,
    /// The tab's width as last laid out, which decides whether the outline
    /// fits.
    width: Rc<Cell<f32>>,
    /// The newest outline read; an older one's result is dropped.
    load: u64,
    _events: Subscription,
}

impl CodeView {
    /// The cursor's line, and how many lines the file has, once read.
    #[cfg(test)]
    pub(crate) fn shown(&self, cx: &App) -> (u32, Option<usize>) {
        let editor = self.editor.read(cx);
        (
            editor.cursor_line(),
            editor.loaded().then(|| editor.line_count()),
        )
    }
}

impl HerdrWindow {
    /// Opens `target` in a code tab of the focused workspace, or brings
    /// back the tab already showing that file, at the target's line.
    pub(crate) fn open_code_view(
        &mut self,
        target: &EditorTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((scope, workspace)) = self.browser_key() else {
            self.show_flash(Flash::warning("Open a workspace first"), cx);
            return;
        };
        let Some(path) = target.path.to_str().map(str::to_owned) else {
            self.show_flash(
                Flash::warning(crate::Error::InvalidCodeFile.to_string()),
                cx,
            );
            return;
        };
        let existing = cx.try_global::<Store>().and_then(|store| {
            store
                .in_workspace(&scope, &workspace)
                .find(|tab| {
                    matches!(&tab.location, Some(Location::Code { file }) if file.path == path)
                })
                .map(|tab| tab.id)
        });
        let file = CodeFile {
            path,
            line: target.line,
        };
        let opened = existing.or_else(|| {
            Store::update(cx, |store| {
                store.open(scope, &workspace, Some(Location::Code { file }), None)
            })
        });
        let Some(id) = opened else {
            self.show_flash(Flash::warning("Too many tabs are open"), cx);
            return;
        };
        let made = self.ensure_code_view(id, cx);
        let Some(view) = self.code_views.get(&id) else {
            return;
        };
        let (editor, focus) = (view.editor.clone(), view.focus.clone());
        editor.update(cx, |editor, cx| {
            // An agent may have changed the file since, and the line came
            // from a newer index: an open tab without edits reads it again
            // before going there.
            if !made {
                editor.disk_changed(cx);
            }
            if let Some(line) = target.line {
                editor.go_to_line(line, cx);
            }
        });
        if !made {
            self.load_code_view(id, cx);
        }
        self.show_browser_tab(id, window, cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    /// How code tabs draw, from the window's settings.
    fn code_look(&self) -> Look {
        Look {
            font: self.config.terminal.clone(),
            theme: self.theme.clone(),
            line_height: self.code_line_height(),
        }
    }

    /// Makes `id`'s view from its tab, if the window has none yet, and
    /// starts reading its file. Whether it was made.
    fn ensure_code_view(&mut self, id: TabId, cx: &mut Context<Self>) -> bool {
        if self.code_views.contains_key(&id) {
            return false;
        }
        let Some(Location::Code { file }) = cx
            .try_global::<Store>()
            .and_then(|store| store.get(id))
            .and_then(|tab| tab.location.clone())
        else {
            return false;
        };
        let look = self.code_look();
        let editor = cx.new(|cx| {
            let mut editor = CodeEditor::new(file.path.clone().into(), look, cx);
            if let Some(line) = file.line {
                editor.go_to_line(line, cx);
            }
            editor
        });
        let events = cx.subscribe(&editor, move |this, editor, event: &Event, cx| {
            this.code_edit_event(id, &editor, event, cx);
        });
        let focus = editor.read(cx).focus.clone();
        self.code_views.insert(
            id,
            CodeView {
                focus,
                editor,
                outline: None,
                show_outline: true,
                width: Rc::default(),
                load: 0,
                _events: events,
            },
        );
        self.load_code_view(id, cx);
        true
    }

    /// Reads tab `id`'s outline off the UI thread.
    pub(crate) fn load_code_view(&mut self, id: TabId, cx: &mut Context<Self>) {
        let Some(Location::Code { file }) = cx
            .try_global::<Store>()
            .and_then(|store| store.get(id))
            .and_then(|tab| tab.location.clone())
        else {
            return;
        };
        let Some(view) = self.code_views.get_mut(&id) else {
            return;
        };
        view.load += 1;
        let load = view.load;
        let path = std::path::PathBuf::from(&file.path);
        let read = cx
            .background_executor()
            .spawn(async move { Outline::read(&path).map(Arc::new) });
        cx.spawn(async move |this, cx| {
            let outline = read.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(view) = this.code_views.get_mut(&id)
                    && view.load == load
                {
                    // An unreadable file says so in the editor.
                    view.outline = outline.ok();
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Runs every window tick: code tabs of the focused workspace with no
    /// view yet, such as ones restored from the last run, get one; the
    /// views of closed tabs go, their editors saving what is unsaved.
    pub(crate) fn poll_code_views(&mut self, cx: &mut Context<Self>) {
        let Some(store) = cx.try_global::<Store>() else {
            self.code_views.clear();
            return;
        };
        self.code_views.retain(|id, _| store.get(*id).is_some());
        let Some((scope, workspace)) = self.browser_key() else {
            return;
        };
        let missing: Vec<TabId> = store
            .in_workspace(&scope, &workspace)
            .filter(|tab| {
                matches!(tab.location, Some(Location::Code { .. }))
                    && !self.code_views.contains_key(&tab.id)
            })
            .map(|tab| tab.id)
            .collect();
        for id in missing {
            self.ensure_code_view(id, cx);
        }
    }

    /// Opens tab `id`'s file in the terminal editor at the cursor's line,
    /// beside the focused pane, saving the tab's edits first.
    fn open_code_view_in_editor(&mut self, id: TabId, cx: &mut Context<Self>) {
        let Some(Location::Code { file }) = cx
            .try_global::<Store>()
            .and_then(|store| store.get(id))
            .and_then(|tab| tab.location.clone())
        else {
            return;
        };
        let line = self.code_views.get(&id).map(|view| {
            view.editor.update(cx, |editor, cx| {
                if editor.dirty() {
                    editor.save(false, cx);
                }
                editor.cursor_line()
            })
        });
        let target = EditorTarget {
            path: file.path.into(),
            line,
        };
        self.open_in_editor(&target, None, cx);
    }
}

#[cfg(test)]
mod tests;
