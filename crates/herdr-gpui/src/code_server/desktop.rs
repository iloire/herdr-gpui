//! Opening a folder, and files in it, in the VS Code app on this computer,
//! through its `code` command. Blocking: finding the command may run the
//! login shell, so run it off the UI thread.
use super::{Error, cli, error::Result};
use std::path::Path;

/// Opens `paths` in one VS Code window: a folder as its workspace, a file
/// in it. `code` hands them to a running VS Code, or starts one, and exits.
pub(crate) fn open(paths: &[impl AsRef<Path>]) -> Result<()> {
    let program = cli::command().ok_or(Error::NotInstalled)?;
    let mut command = herdr_client::new_command(&program);
    command.args(paths.iter().map(AsRef::as_ref));
    let status = command
        .status()
        .map_err(|source| Error::Spawn { program, source })?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Exited(status))
    }
}
