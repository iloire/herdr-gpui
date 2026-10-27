//! A checkout's listed files as folders, built once per listing off the UI
//! thread, and the rows its open folders show. Folders lead their files, as
//! file browsers list them, and each folder counts the changed files under
//! it so a closed folder still says the agent was in there.

use herdr_vcs::code_index::Index;
use std::collections::{HashMap, HashSet};

/// One folder or file. Node 0 is the checkout itself.
#[derive(Debug)]
pub(crate) struct Node {
    pub(crate) name: String,
    /// Relative to the checkout with `/` between parts; empty for the root.
    pub(crate) path: String,
    pub(crate) entry: Entry,
    /// Changed files at or under this node.
    pub(crate) changed: u32,
}

#[derive(Debug)]
pub(crate) enum Entry {
    /// Child nodes, folders first, each part by name.
    Folder(Vec<usize>),
    /// The file's place in the index's files.
    File(usize),
}

/// One shown node and how deep it sits under the checkout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) node: usize,
    pub(crate) depth: u16,
}

#[derive(Debug)]
pub(crate) struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    /// The folders of `index`'s files. Blocking on a large checkout, so it
    /// runs where the listing was read.
    pub(crate) fn new(index: &Index) -> Self {
        let mut nodes = vec![Node {
            name: String::new(),
            path: String::new(),
            entry: Entry::Folder(Vec::new()),
            changed: 0,
        }];
        let mut folders: HashMap<String, usize> = HashMap::new();
        let mut files = HashMap::with_capacity(index.files().len());
        for (file, name) in index.files().iter().enumerate() {
            let mut parent = 0;
            for path in ancestors(name) {
                parent = match folders.get(path) {
                    Some(&node) => node,
                    None => {
                        let node = push(&mut nodes, parent, path, Entry::Folder(Vec::new()));
                        folders.insert(path.to_owned(), node);
                        node
                    }
                };
            }
            files.insert(file, push(&mut nodes, parent, name, Entry::File(file)));
        }
        for change in index.changes() {
            let Some(&node) = files.get(&change.file) else {
                continue;
            };
            nodes[0].changed += 1;
            nodes[node].changed += 1;
            for folder in ancestors(&index.files()[change.file]) {
                if let Some(&folder) = folders.get(folder) {
                    nodes[folder].changed += 1;
                }
            }
        }
        let mut tree = Self { nodes };
        tree.sort();
        tree
    }

    pub(crate) fn node(&self, node: usize) -> Option<&Node> {
        self.nodes.get(node)
    }

    /// The rows shown with the folders in `expanded` open, top first.
    pub(crate) fn rows(&self, expanded: &HashSet<String>) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut stack: Vec<Row> = self
            .children(0)
            .iter()
            .rev()
            .map(|&node| Row { node, depth: 0 })
            .collect();
        while let Some(row) = stack.pop() {
            rows.push(row);
            let node = &self.nodes[row.node];
            if matches!(node.entry, Entry::Folder(_)) && expanded.contains(&node.path) {
                stack.extend(self.children(row.node).iter().rev().map(|&node| Row {
                    node,
                    depth: row.depth.saturating_add(1),
                }));
            }
        }
        rows
    }

    /// The rows of the files `query` matches and the folders holding them,
    /// every one of those open. The query is matched without case against
    /// a file's name, or its whole path once it holds a `/`.
    pub(crate) fn matching(&self, query: &str) -> Vec<Row> {
        let query = query.to_lowercase();
        let mut rows = Vec::new();
        self.collect_matching(0, 0, &query, &mut rows);
        rows
    }

    /// Appends the matching rows under `node`; whether it found any.
    fn collect_matching(&self, node: usize, depth: u16, query: &str, rows: &mut Vec<Row>) -> bool {
        let start = rows.len();
        for &child in self.children(node) {
            let row = Row { node: child, depth };
            let found = &self.nodes[child];
            match found.entry {
                Entry::Folder(_) => {
                    rows.push(row);
                    if !self.collect_matching(child, depth.saturating_add(1), query, rows) {
                        rows.pop();
                    }
                }
                Entry::File(_) => {
                    let text = if query.contains('/') {
                        &found.path
                    } else {
                        &found.name
                    };
                    if text.to_lowercase().contains(query) {
                        rows.push(row);
                    }
                }
            }
        }
        rows.len() > start
    }

    fn children(&self, node: usize) -> &[usize] {
        match &self.nodes[node].entry {
            Entry::Folder(children) => children,
            Entry::File(_) => &[],
        }
    }

    fn sort(&mut self) {
        for node in 0..self.nodes.len() {
            let Entry::Folder(children) = &self.nodes[node].entry else {
                continue;
            };
            let mut children = children.clone();
            children.sort_by_cached_key(|&child| {
                let child = &self.nodes[child];
                (
                    matches!(child.entry, Entry::File(_)),
                    child.name.to_lowercase(),
                    child.name.clone(),
                )
            });
            self.nodes[node].entry = Entry::Folder(children);
        }
    }
}

/// Adds a node named after the last part of `path` under `parent`.
fn push(nodes: &mut Vec<Node>, parent: usize, path: &str, entry: Entry) -> usize {
    let node = nodes.len();
    let name = path.rsplit_once('/').map_or(path, |(_, name)| name);
    nodes.push(Node {
        name: name.to_owned(),
        path: path.to_owned(),
        entry,
        changed: 0,
    });
    if let Entry::Folder(children) = &mut nodes[parent].entry {
        children.push(node);
    }
    node
}

/// The folders holding `path`, outermost first: `a/b/c` gives `a`, `a/b`.
pub(crate) fn ancestors(path: &str) -> impl Iterator<Item = &str> {
    path.match_indices('/').map(move |(end, _)| &path[..end])
}
