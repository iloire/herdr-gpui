#![allow(clippy::unwrap_used)]

use super::{Files, Index, Instant, Listing, Read, Status, Tree, relative, tree::Row};
use herdr_vcs::code_index::Change;
use std::{
    collections::HashSet,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

mod filter;
mod follow;
mod menu;
mod tree;

const FILES: &[&str] = &[
    "Cargo.toml",
    "README.md",
    "crates/app/src/main.rs",
    "crates/app/src/ui/view.rs",
    "crates/app/Cargo.toml",
    "docs/guide.md",
];

fn change(file: usize) -> Change {
    Change {
        file,
        counts: Some((3, 1)),
        new: false,
        modified: None,
    }
}

/// `FILES` under `/repo`, with `changed` (indexes into `FILES`) uncommitted.
fn index(changed: &[usize]) -> Index {
    Index::of(Path::new("/repo"), FILES, Vec::new())
        .with_changes(changed.iter().copied().map(change).collect())
}

impl Listing {
    /// `index` laid out as a read would.
    pub(crate) fn of(index: Index) -> Self {
        let tree = Tree::new(&index);
        Self {
            index: Arc::new(index),
            tree: Arc::new(tree),
        }
    }
}

fn listing(changed: &[usize]) -> Listing {
    Listing::of(index(changed))
}

/// The shown rows as indented paths.
fn shown(files: &Files) -> Vec<String> {
    let tree = &files.listing.as_ref().unwrap().tree;
    files
        .rows
        .iter()
        .map(|row| {
            let node = tree.node(row.node).unwrap();
            format!("{}{}", "  ".repeat(usize::from(row.depth)), node.name)
        })
        .collect()
}
