use super::*;
use crate::files::tree::{Entry, ancestors};

fn names(tree: &Tree, rows: &[Row]) -> Vec<String> {
    rows.iter()
        .map(|row| {
            let node = tree.node(row.node).unwrap();
            format!("{}{}", "  ".repeat(usize::from(row.depth)), node.name)
        })
        .collect()
}

#[test]
fn folders_lead_their_files_and_stay_closed_until_opened() {
    let tree = Tree::new(&index(&[]));
    let closed = tree.rows(&HashSet::new());
    assert_eq!(
        names(&tree, &closed),
        ["crates", "docs", "Cargo.toml", "README.md"]
    );
    let open: HashSet<String> = ["crates", "crates/app", "crates/app/src"]
        .map(str::to_owned)
        .into();
    assert_eq!(
        names(&tree, &tree.rows(&open)),
        [
            "crates",
            "  app",
            "    src",
            "      ui",
            "      main.rs",
            "    Cargo.toml",
            "docs",
            "Cargo.toml",
            "README.md",
        ]
    );
}

#[test]
fn an_open_folder_inside_a_closed_one_stays_hidden() {
    let tree = Tree::new(&index(&[]));
    let open: HashSet<String> = ["crates/app"].map(str::to_owned).into();
    assert_eq!(
        names(&tree, &tree.rows(&open)),
        ["crates", "docs", "Cargo.toml", "README.md"]
    );
}

#[test]
fn folders_count_the_changed_files_under_them() {
    // main.rs and view.rs under crates/app/src, guide.md under docs.
    let tree = Tree::new(&index(&[2, 3, 5]));
    let changed = |path: &str| {
        let open: HashSet<String> = ancestors(&format!("{path}/x")).map(str::to_owned).collect();
        tree.rows(&open)
            .iter()
            .map(|row| tree.node(row.node).unwrap())
            .find(|node| node.path == path)
            .map(|node| node.changed)
    };
    assert_eq!(tree.node(0).unwrap().changed, 3);
    assert_eq!(changed("crates"), Some(2));
    assert_eq!(changed("crates/app/src"), Some(2));
    assert_eq!(changed("crates/app/src/ui"), Some(1));
    assert_eq!(changed("docs"), Some(1));
    assert_eq!(changed("README.md"), Some(0));
}

#[test]
fn files_keep_their_place_in_the_index() {
    let tree = Tree::new(&index(&[]));
    let open: HashSet<String> = ["docs"].map(str::to_owned).into();
    let guide = tree
        .rows(&open)
        .iter()
        .map(|row| tree.node(row.node).unwrap())
        .find(|node| node.name == "guide.md")
        .unwrap();
    assert!(matches!(guide.entry, Entry::File(5)));
    assert_eq!(guide.path, "docs/guide.md");
}

#[test]
fn ancestors_run_outermost_first() {
    assert_eq!(ancestors("a/b/c.rs").collect::<Vec<_>>(), ["a", "a/b"]);
    assert_eq!(ancestors("c.rs").count(), 0);
}
