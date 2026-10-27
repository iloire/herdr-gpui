//! GitHub Desktop on this computer: whether it is installed, and opening a
//! folder in it. macOS only: there it is an app bundle `open` can hand a
//! folder to; elsewhere it has no command this app could rely on.
use crate::{HerdrWindow, window::Flash};
use gpui::{App, Context};
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// Where the app is installed: system-wide, else for this user alone.
const BUNDLE: &str = "Applications/GitHub Desktop.app";

/// The installed app, once looked for. Read on the UI thread, so it is only
/// filled off it, by [`install`].
static APP: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Looks for the app off the UI thread, so menus can offer it.
pub(crate) fn install(cx: &App) {
    cx.background_executor()
        .spawn(async {
            let home = herdr_client::local_path::home_dir();
            APP.get_or_init(|| find([Some(Path::new("/")), home.as_deref()]));
        })
        .detach();
}

/// Whether GitHub Desktop was found; `false` until the look has finished.
pub(crate) fn installed() -> bool {
    APP.get().is_some_and(Option::is_some)
}

/// The first of `roots` with the app under it.
fn find<'a>(roots: impl IntoIterator<Item = Option<&'a Path>>) -> Option<PathBuf> {
    roots
        .into_iter()
        .flatten()
        .map(|root| root.join(BUNDLE))
        .find(|app| app.is_dir())
}

impl HerdrWindow {
    /// Opens `folder` in GitHub Desktop, off the UI thread; it shows the
    /// repository there, or offers to add it.
    pub(crate) fn open_in_github_desktop(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        let Some(Some(app)) = APP.get().cloned() else {
            return;
        };
        let opened = cx.background_executor().spawn(async move {
            herdr_client::new_command("open")
                .arg("-a")
                .arg(&app)
                .arg(&folder)
                .status()
                .map(|status| status.success())
        });
        cx.spawn(async move |this, cx| {
            match opened.await {
                Ok(true) => return,
                Ok(false) => tracing::warn!("GitHub Desktop did not take the folder"),
                Err(error) => tracing::warn!("Unable to open GitHub Desktop: {error}"),
            }
            let _ = this.update(cx, |this, cx| {
                this.show_flash(
                    Flash::warning("GitHub Desktop could not open the folder"),
                    cx,
                );
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests;
