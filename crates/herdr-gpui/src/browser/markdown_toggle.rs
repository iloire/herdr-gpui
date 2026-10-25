//! A markdown file shown either way in one tab: rendered, as a local page
//! (`markdown`), or as its source, in a code tab. Toggling swaps the tab's
//! location in place, so the tab keeps its place in its strip, and the
//! choice is saved with the tab like any location. Builds without pages
//! (Linux) have nothing to render with, so they only ever show the source.

use super::{LocalFile, Location, Store, TabId};
use crate::{HerdrWindow, code_view::CodeFile, window::Flash};
use gpui::{Context, Window};
use std::path::Path;

/// Whether `path` is a markdown file this build can render.
#[cfg(any(target_os = "macos", windows))]
pub(crate) fn renders(path: &Path) -> bool {
    super::markdown::is_markdown(path)
}

#[cfg(not(any(target_os = "macos", windows)))]
pub(crate) fn renders(_: &Path) -> bool {
    false
}

/// The markdown file `location` shows, rendered or as source.
fn markdown_file(location: &Location) -> Option<std::path::PathBuf> {
    let path = match location {
        Location::Local { file } => file.path(),
        Location::Code { file } => file.path.clone().into(),
        _ => return None,
    };
    renders(&path).then_some(path)
}

/// `path` rendered, as a page may read it.
fn rendered(path: &Path) -> crate::Result<Location> {
    let home = crate::config::home().ok();
    Ok(Location::Local {
        file: LocalFile::new(path, home.as_deref())?,
    })
}

/// `path` as source, in a code tab.
fn source(path: &Path) -> Option<Location> {
    Some(Location::Code {
        file: CodeFile {
            path: path.to_str()?.to_owned(),
            line: None,
        },
    })
}

impl HerdrWindow {
    /// Whether tab `id` shows a markdown file this build can render either
    /// way, and so offers the toggle.
    pub(crate) fn markdown_toggles(&self, id: TabId, cx: &gpui::App) -> bool {
        cx.try_global::<Store>()
            .and_then(|store| store.get(id))
            .and_then(|tab| tab.location.as_ref())
            .and_then(markdown_file)
            .is_some()
    }

    /// Shows tab `id`'s markdown file rendered when it shows the source, and
    /// the source when it shows it rendered.
    pub(crate) fn toggle_markdown(
        &mut self,
        id: TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(location) = cx
            .try_global::<Store>()
            .and_then(|store| store.get(id))
            .and_then(|tab| tab.location.clone())
        else {
            return;
        };
        let Some(path) = markdown_file(&location) else {
            return;
        };
        let next = match location {
            Location::Code { .. } => match rendered(&path) {
                Ok(next) => next,
                Err(error) => {
                    self.show_flash(Flash::warning(error.to_string()), cx);
                    return;
                }
            },
            _ => match source(&path) {
                Some(next) => next,
                None => return,
            },
        };
        self.swap_location(id, next, window, cx);
    }

    /// Opens markdown file `path` rendered in the focused workspace, turning
    /// a tab already on it, either way, rather than opening another.
    pub(crate) fn open_markdown(
        &mut self,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((scope, workspace)) = self.browser_key() else {
            self.show_flash(Flash::warning("Open a workspace first"), cx);
            return;
        };
        let next = match rendered(path) {
            Ok(next) => next,
            Err(error) => {
                self.show_flash(Flash::warning(error.to_string()), cx);
                return;
            }
        };
        let existing = cx.try_global::<Store>().and_then(|store| {
            store
                .in_workspace(&scope, &workspace)
                .find(|tab| {
                    tab.location
                        .as_ref()
                        .and_then(markdown_file)
                        .is_some_and(|open| open == path)
                })
                .map(|tab| tab.id)
        });
        if let Some(id) = existing {
            self.swap_location(id, next, window, cx);
            return;
        }
        match Store::update(cx, |store| store.open(scope, &workspace, Some(next), None)) {
            Some(id) => self.show_browser_tab(id, window, cx),
            None => self.show_flash(Flash::warning("Too many tabs are open"), cx),
        }
    }

    /// Points tab `id` at `next` and shows it: the page a rendered file had
    /// goes, so the code view draws in its place, and a rendered file gets
    /// its page when shown.
    fn swap_location(
        &mut self,
        id: TabId,
        next: Location,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let to_page = next.is_page();
        Store::update(cx, |store| store.visited(id, Some(next), None));
        if to_page {
            self.code_views.remove(&id);
        } else {
            #[cfg(any(target_os = "macos", windows))]
            self.browser.pages.close(id);
            self.browser.failed.remove(&id);
            self.poll_code_views(cx);
        }
        self.show_browser_tab(id, window, cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
