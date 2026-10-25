//! Folders on this computer: how a path is written for the user, and opening
//! one in the file manager.

#[cfg(any(target_os = "macos", target_os = "linux"))]
use gpui::App;
use std::path::Path;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::path::PathBuf;

/// `path` with the home folder written `~`. Only meaningful for a path on
/// this computer: another host's home is not this one's.
pub(crate) fn home_relative(path: &str) -> String {
    herdr_client::local_path::home_dir()
        .and_then(|home| {
            let rest = Path::new(path).strip_prefix(&home).ok()?;
            Some(format!("~/{}", rest.display()))
        })
        .unwrap_or_else(|| path.to_owned())
}

/// Reveals `path` in the file manager, or opens it in the default editor.
/// Spawned on the background executor: waiting on the helper never blocks the UI.
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn open_path(reveal: bool, path: PathBuf, cx: &mut App) {
    cx.background_executor()
        .spawn(async move {
            #[cfg(target_os = "macos")]
            let mut command = {
                let mut command = herdr_client::new_command("open");
                if reveal {
                    command.arg("-R");
                }
                command.arg(&path);
                command
            };
            #[cfg(target_os = "linux")]
            let mut command = {
                let mut command = herdr_client::new_command("xdg-open");
                command.arg(if reveal {
                    path.parent().unwrap_or(&path)
                } else {
                    &path
                });
                command
            };
            if let Err(error) = command.status() {
                tracing::warn!("Unable to open {}: {error}", path.display());
            }
        })
        .detach();
}
