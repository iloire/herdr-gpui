//! A dispatch on an item whose branch already has a checkout, as when an
//! earlier run's checkout outlived its agent. Branch names are the item's
//! own, so Herdr refuses to create the checkout a second time; the agent
//! starts in a new tab of the repository's folder opened on the existing
//! one instead. That checkout is not the run's to remove on failure.

use super::{Error, Result, herdr};
use herdr_remote::teleport::{CreatedPane, Envelope, Host};
use std::sync::atomic::AtomicBool;

/// The linked checkout of `branch` that the repository of `workspace_id`
/// lists, when it has one on disk.
pub(super) fn checkout(
    host: &Host,
    workspace_id: &str,
    branch: &str,
    cancelled: &AtomicBool,
) -> Result<Option<String>> {
    const LISTING: &str = "Listing the worktrees";
    let output = herdr(
        host,
        &["worktree", "list", "--workspace", workspace_id],
        LISTING,
        cancelled,
    )?;
    let listed = serde_json::from_slice::<Envelope<WorktreeList>>(&output)
        .map_err(|source| Error::OutputJson {
            operation: LISTING,
            source,
        })?
        .result;
    Ok(listed
        .worktrees
        .into_iter()
        .find(|tree| {
            tree.is_linked_worktree && !tree.is_prunable && tree.branch.as_deref() == Some(branch)
        })
        .map(|tree| tree.path))
}

/// A new tab of the folder `workspace_id` with its shell in `path`.
pub(super) fn open_tab(
    host: &Host,
    workspace_id: &str,
    path: &str,
    cancelled: &AtomicBool,
) -> Result<CreatedPane> {
    const OPENING: &str = "Opening the agent's tab";
    let output = herdr(
        host,
        &[
            "tab",
            "create",
            "--workspace",
            workspace_id,
            "--cwd",
            path,
            "--no-focus",
        ],
        OPENING,
        cancelled,
    )?;
    serde_json::from_slice::<Envelope<TabCreated>>(&output)
        .map(|envelope| envelope.result.root_pane)
        .map_err(|source| Error::OutputJson {
            operation: OPENING,
            source,
        })
}

/// `worktree_list`, as `worktree list` prints it.
#[derive(serde::Deserialize)]
struct WorktreeList {
    worktrees: Vec<Listed>,
}

#[derive(serde::Deserialize)]
struct Listed {
    path: String,
    branch: Option<String>,
    #[serde(default)]
    is_linked_worktree: bool,
    #[serde(default)]
    is_prunable: bool,
}

/// `tab create`'s result.
#[derive(serde::Deserialize)]
struct TabCreated {
    root_pane: CreatedPane,
}
