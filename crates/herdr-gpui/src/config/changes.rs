//! `[changes]`: what the Changes section reads besides the checkouts this
//! machine's workspaces and panes are in. `repos` names checkouts to show
//! whether or not anything is open in them; `roots` names folders whose
//! repositories, one level down, are all read. `~/` is the home folder.
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ChangesConfig {
    pub repos: Vec<String>,
    pub roots: Vec<String>,
}

impl ChangesConfig {
    pub(crate) fn repo_paths(&self) -> Vec<PathBuf> {
        self.repos.iter().filter_map(|path| expand(path)).collect()
    }

    pub(crate) fn root_paths(&self) -> Vec<PathBuf> {
        self.roots.iter().filter_map(|path| expand(path)).collect()
    }
}

/// `path` with a leading `~/` or `~` read as the home folder. A relative
/// path names nothing: the app has no working directory to resolve it from.
pub(crate) fn expand(path: &str) -> Option<PathBuf> {
    let expanded = match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => {
            herdr_client::local_path::home_dir()?.join(rest.trim_start_matches('/'))
        }
        _ => PathBuf::from(path),
    };
    expanded.is_absolute().then_some(expanded)
}
