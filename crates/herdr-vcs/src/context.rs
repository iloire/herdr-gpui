//! What one directory's checkout is up to, for the context strip at the
//! foot of a tab: what the Changes section reads of it, the other worktrees
//! of its repository, and what Git status leaves out: an operation stopped
//! halfway, stashed work, and a git-crypt repository whose key is missing.
//! Those last three are plain file reads in the checkout's Git directories,
//! so a strip can read them as often as it reads status. Read-only, run by
//! the caller off the UI thread; nothing contacts a remote.

use crate::{
    changes::{self, Checkout},
    host::Session,
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

/// Other worktrees read per directory, however many the repository has.
pub const MAX_WORKTREES: usize = 8;

/// A directory's checkout as last read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    pub checkout: Checkout,
    pub state: State,
    /// The repository's other worktrees whose folders are still there, at
    /// most [`MAX_WORKTREES`] of them read.
    pub worktrees: Vec<Checkout>,
}

/// What a checkout is in the middle of.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub stopped: Option<Stopped>,
    /// Entries in the repository's stash, which its worktrees share.
    pub stashes: usize,
    /// `.gitattributes` sends files through git-crypt and no key is there
    /// to decrypt them, so they read as ciphertext.
    pub crypt_locked: bool,
}

/// An operation Git stopped halfway, waiting on the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stopped {
    Bisect,
    CherryPick,
    Merge,
    /// At `step` of `of`, when Git recorded them.
    Rebase {
        step: Option<(u32, u32)>,
    },
    Revert,
}

impl Stopped {
    /// A short label, such as `rebase 3/7`.
    pub fn label(self) -> String {
        match self {
            Self::Bisect => "bisect".into(),
            Self::CherryPick => "cherry-pick".into(),
            Self::Merge => "merge".into(),
            Self::Rebase {
                step: Some((step, of)),
            } => format!("rebase {step}/{of}"),
            Self::Rebase { step: None } => "rebase".into(),
            Self::Revert => "revert".into(),
        }
    }
}

/// Reads the checkout `directory` is in, from any folder inside it. A
/// directory outside Git is an error.
pub fn read(
    directory: &str,
    deadline: Instant,
    cancelled: &impl Fn() -> bool,
) -> crate::Result<Context> {
    // The state beside the checkout reads Git's own files, so this machine's.
    let session = &mut Session::local();
    let located = changes::locate(session, directory, deadline, cancelled)?;
    let state = state(Path::new(&located.path), Path::new(&located.common_dir));
    let path = located.path.clone();
    let checkout = changes::read(session, located, deadline, cancelled)?;
    // The other worktrees are a nicety: a failure keeps the checkout.
    let others = changes::worktrees(session, &path, deadline, cancelled).unwrap_or_default();
    let mut worktrees = Vec::new();
    for other in others.iter().filter(|other| **other != path) {
        if worktrees.len() == MAX_WORKTREES || cancelled() {
            break;
        }
        let read = changes::locate(session, other, deadline, cancelled)
            .and_then(|located| changes::read(session, located, deadline, cancelled));
        if let Ok(read) = read {
            worktrees.push(read);
        }
    }
    Ok(Context {
        checkout,
        state,
        worktrees,
    })
}

/// What the checkout at `checkout`, whose repository keeps its shared
/// files in `common_dir`, is in the middle of.
pub fn state(checkout: &Path, common_dir: &Path) -> State {
    let git_dir = git_dir(checkout).unwrap_or_else(|| common_dir.to_path_buf());
    State {
        stopped: stopped(&git_dir),
        stashes: stashes(common_dir),
        crypt_locked: crypt_locked(checkout, &git_dir, common_dir),
    }
}

/// The checkout's own Git directory: `.git` itself, or the folder a linked
/// worktree's `.git` file points at.
fn git_dir(checkout: &Path) -> Option<PathBuf> {
    let dot_git = checkout.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    let text = fs::read_to_string(&dot_git).ok()?;
    let target = text.lines().next()?.strip_prefix("gitdir:")?.trim();
    Some(checkout.join(target))
}

fn stopped(git_dir: &Path) -> Option<Stopped> {
    let number =
        |path: PathBuf| -> Option<u32> { fs::read_to_string(path).ok()?.trim().parse().ok() };
    // `rebase -i` and the merge backend keep `rebase-merge`; the apply
    // backend and `git am` keep `rebase-apply`.
    for (folder, step, of) in [
        ("rebase-merge", "msgnum", "end"),
        ("rebase-apply", "next", "last"),
    ] {
        let folder = git_dir.join(folder);
        if folder.is_dir() {
            let step = number(folder.join(step)).zip(number(folder.join(of)));
            return Some(Stopped::Rebase { step });
        }
    }
    [
        ("MERGE_HEAD", Stopped::Merge),
        ("CHERRY_PICK_HEAD", Stopped::CherryPick),
        ("REVERT_HEAD", Stopped::Revert),
        ("BISECT_LOG", Stopped::Bisect),
    ]
    .into_iter()
    .find(|(file, _)| git_dir.join(file).is_file())
    .map(|(_, stopped)| stopped)
}

/// The stash's reflog holds one line per entry.
fn stashes(common_dir: &Path) -> usize {
    fs::read_to_string(common_dir.join("logs/refs/stash"))
        .map(|text| text.lines().filter(|line| !line.trim().is_empty()).count())
        .unwrap_or(0)
}

/// git-crypt keeps its unlocked key under the Git directory: the common one
/// in current releases, the worktree's own in older ones.
fn crypt_locked(checkout: &Path, git_dir: &Path, common_dir: &Path) -> bool {
    let Ok(attributes) = fs::read_to_string(checkout.join(".gitattributes")) else {
        return false;
    };
    let encrypted = attributes
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .any(|line| {
            line.split_whitespace()
                .any(|field| field == "filter=git-crypt")
        });
    let key = |dir: &Path| dir.join("git-crypt/keys/default").is_file();
    encrypted && !key(common_dir) && !key(git_dir)
}

#[cfg(test)]
mod tests;
