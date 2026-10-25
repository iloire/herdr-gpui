#![allow(clippy::unwrap_used)]

use super::{Files, Index, Instant, Listing, Read, Status, Tree, relative, tree::Row};
use herdr_vcs::code_index::Change;
use std::{
    collections::HashSet,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

mod follow;
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

fn listing(changed: &[usize]) -> Listing {
    let index = index(changed);
    let tree = Tree::new(&index);
    Listing {
        index: Arc::new(index),
        tree: Arc::new(tree),
    }
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
