#![allow(clippy::unwrap_used)]

use super::{
    Changes, Rank, Read, Shown, Targets, Workspace, discover, group, rank, repo_name, section::age,
};
use herdr_vcs::changes::{Checkout, FileChange, Kind, Unmerged};
use std::{path::Path, process::Command, time::Duration};

fn workspace(id: &str) -> Workspace {
    Workspace {
        id: id.into(),
        label: format!("label {id}"),
    }
}

fn checkout(path: &str, common: &str) -> Checkout {
    Checkout {
        path: path.into(),
        common_dir: common.into(),
        branch: Some("main".into()),
        ..Checkout::default()
    }
}

fn dirty(mut checkout: Checkout) -> Checkout {
    checkout.files = vec![FileChange {
        path: "a.rs".into(),
        kind: Kind::Modified,
        additions: Some(2),
        deletions: Some(1),
    }];
    checkout.total_files = 1;
    checkout
}

fn shown(checkout: Checkout, workspaces: &[&str]) -> Shown {
    Shown {
        checkout,
        workspaces: workspaces.iter().map(|id| workspace(id)).collect(),
    }
}

#[test]
fn a_checkout_ranks_by_what_it_asks_of_the_user() {
    let clean = checkout("/r", "/r/.git");
    assert_eq!(rank(&clean), Rank::Clean);
    let behind = Checkout {
        behind: 3,
        ..clean.clone()
    };
    assert_eq!(rank(&behind), Rank::Behind);
    let unmerged = Checkout {
        unmerged: Some(Unmerged {
            base: "origin/main".into(),
            count: 2,
        }),
        ..behind.clone()
    };
    assert_eq!(rank(&unmerged), Rank::Unmerged);
    let unpushed = Checkout {
        ahead: 1,
        ..unmerged.clone()
    };
    assert_eq!(rank(&unpushed), Rank::Unpushed);
    assert_eq!(rank(&dirty(unpushed)), Rank::Dirty);
    // Merged into the default branch already: nothing to do.
    let merged = Checkout {
        unmerged: Some(Unmerged {
            base: "origin/main".into(),
            count: 0,
        }),
        ..clean
    };
    assert_eq!(rank(&merged), Rank::Clean);
}

#[test]
fn worktrees_group_under_their_repository_and_what_needs_work_leads() {
    let repos = group(vec![
        shown(checkout("/code/zeta", "/code/zeta/.git"), &["1"]),
        shown(checkout("/code/app", "/code/app/.git"), &["2"]),
        shown(dirty(checkout("/code/app-feature", "/code/app/.git")), &[]),
        shown(
            Checkout {
                ahead: 2,
                ..checkout("/code/Beta", "/code/Beta/.git")
            },
            &["4"],
        ),
    ]);
    let names: Vec<_> = repos.iter().map(|repo| repo.name.as_str()).collect();
    assert_eq!(names, ["app", "Beta", "zeta"]);
    let app: Vec<_> = repos[0]
        .checkouts
        .iter()
        .map(|shown| shown.checkout.path.as_str())
        .collect();
    assert_eq!(app, ["/code/app-feature", "/code/app"]);
}

#[test]
fn the_bar_counts_dirty_checkouts_and_the_filter_counts_clean_ones() {
    let mut changes = Changes::default();
    assert_eq!((changes.dirty(), changes.clean()), (0, 0));
    changes.read_for_test(Read {
        repos: group(vec![
            shown(dirty(checkout("/a", "/a/.git")), &["1"]),
            shown(dirty(checkout("/a-2", "/a/.git")), &[]),
            shown(checkout("/b", "/b/.git"), &["3"]),
        ]),
        failed: Vec::new(),
    });
    assert_eq!((changes.dirty(), changes.clean()), (2, 1));
    changes.read_for_test(Read::default());
    assert_eq!(changes.dirty(), 0);
}

#[test]
fn an_open_folder_is_kept_once_with_every_workspace_open_in_it() {
    let mut targets = Targets::default();
    targets.add_open("/code/app", &workspace("1"));
    targets.add_open("/code/app", &workspace("1"));
    targets.add_open("/code/app", &workspace("2"));
    targets.add_open("", &workspace("3"));
    targets.add_open("relative", &workspace("3"));
    assert_eq!(
        targets.open,
        [("/code/app".to_owned(), vec![workspace("1"), workspace("2")])]
    );
}

#[test]
fn a_repository_is_named_by_its_folder() {
    assert_eq!(repo_name("/code/herdr-gpui/.git"), "herdr-gpui");
    assert_eq!(repo_name("/srv/git/project.git"), "project");
    assert_eq!(repo_name("/srv/git/bare"), "bare");
}

#[test]
fn ages_read_in_minutes_hours_then_days() {
    assert_eq!(age(Duration::from_secs(20)), "just now");
    assert_eq!(age(Duration::from_secs(5 * 60)), "5 min ago");
    assert_eq!(age(Duration::from_secs(47 * 3600)), "47 h ago");
    assert_eq!(age(Duration::from_secs(3 * 86_400)), "3 d ago");
}

fn git(directory: &Path, args: &[&str]) {
    let result = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn repository(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    git(path, &["init", "-q", "-b", "main"]);
    std::fs::write(path.join("a"), "a\n").unwrap();
    git(path, &["add", "a"]);
    git(path, &["commit", "-qm", "first"]);
}

fn canonical(path: &str) -> std::path::PathBuf {
    Path::new(path).canonicalize().unwrap()
}

#[test]
fn a_pass_finds_panes_worktrees_named_repos_and_scanned_folders() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("code");
    let app = root.join("app");
    let lib = root.join("lib");
    let notes = home.path().join("notes");
    let linked = home.path().join("app-agent");
    for path in [&app, &lib, &notes] {
        repository(path);
    }
    git(
        &app,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "agent",
            linked.to_str().unwrap(),
        ],
    );
    std::fs::write(linked.join("a"), "changed\n").unwrap();
    std::fs::create_dir(app.join("src")).unwrap();
    let outside = home.path().join("plain");
    std::fs::create_dir(&outside).unwrap();
    let mut targets = Targets {
        repos: vec![notes.clone(), home.path().join("missing")],
        roots: vec![root.clone()],
        ..Targets::default()
    };
    // A pane deep in the checkout and one outside Git.
    targets.add_open(app.join("src").to_str().unwrap(), &workspace("w1"));
    targets.add_open(outside.to_str().unwrap(), &workspace("w2"));

    let read = discover(&targets, &|| false);
    let mut found: Vec<_> = read
        .repos
        .iter()
        .flat_map(|repo| &repo.checkouts)
        .map(|shown| {
            (
                canonical(&shown.checkout.path),
                shown
                    .workspaces
                    .iter()
                    .map(|workspace| workspace.id.as_str())
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    found.sort();
    let mut expected = vec![
        (app.canonicalize().unwrap(), vec!["w1"]),
        (lib.canonicalize().unwrap(), vec![]),
        (notes.canonicalize().unwrap(), vec![]),
        // The worktree no pane is in comes from its repository.
        (linked.canonicalize().unwrap(), vec![]),
    ];
    expected.sort();
    assert_eq!(found, expected);
    // The app repository leads: its worktree has uncommitted work.
    assert_eq!(read.repos[0].name, "app");
    assert_eq!(read.repos[0].checkouts[0].rank(), Rank::Dirty);
    // Only what the config names reports a failure, not the plain pane.
    let failed: Vec<_> = read.failed.iter().map(|failed| &failed.label).collect();
    assert_eq!(failed.len(), 1);
    assert!(failed[0].ends_with("missing"), "{failed:?}");
}

#[test]
fn a_cancelled_pass_reads_nothing() {
    let directory = tempfile::tempdir().unwrap();
    repository(directory.path());
    let mut targets = Targets::default();
    targets.add_open(directory.path().to_str().unwrap(), &workspace("w1"));
    let read = discover(&targets, &|| true);
    assert!(read.repos.is_empty() && read.failed.is_empty());
}

#[test]
fn files_sort_into_code_tests_docs_then_noise() {
    use super::noise::{Group, group_of};
    let extra = [
        "*.blend".to_owned(),
        "renders/".to_owned(),
        "notes.db".to_owned(),
    ];
    for (path, group) in [
        ("src/main.rs", Group::Code),
        ("cad/base.py", Group::Code),
        ("src/changes/tests.rs", Group::Tests),
        ("tests/peek.rs", Group::Tests),
        ("web/app.test.ts", Group::Tests),
        ("pkg/scan_test.go", Group::Tests),
        ("test_scan.py", Group::Tests),
        ("README.md", Group::Docs),
        ("docs/plan.toml", Group::Docs),
        ("LICENSE", Group::Docs),
        ("Cargo.lock", Group::Noise),
        ("models/Lamp v2.STL", Group::Noise),
        ("prints/lamp.gcode", Group::Noise),
        ("web/dist/app.js", Group::Noise),
        ("web/app.min.js", Group::Noise),
        ("assets/hero.png", Group::Noise),
        ("scene.blend", Group::Noise),
        ("renders/a.exr", Group::Noise),
        ("data/notes.db", Group::Noise),
        // A dotfile is named, not an extension.
        (".gitignore", Group::Code),
    ] {
        assert_eq!(group_of(path, &extra), group, "{path}");
    }
}

#[test]
fn groups_keep_gits_order_and_noise_tallies_by_kind() {
    use super::noise::{Group, grouped, tally};
    let file = |path: &str| FileChange {
        path: path.into(),
        kind: Kind::Untracked,
        additions: None,
        deletions: None,
    };
    let files = [
        file("a.gcode"),
        file("README.md"),
        file("b.stl"),
        file("src/z.rs"),
        file("c.gcode"),
        file("src/a.rs"),
        file("old/"),
    ];
    let groups = grouped(&files, &[]);
    let order: Vec<_> = groups
        .iter()
        .map(|(group, files)| {
            (
                *group,
                files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        order,
        [
            (Group::Code, vec!["src/z.rs", "src/a.rs", "old/"]),
            (Group::Docs, vec!["README.md"]),
            (Group::Noise, vec!["a.gcode", "b.stl", "c.gcode"]),
        ]
    );
    assert_eq!(tally(&groups[2].1), "gcode ×2, stl ×1");
}

#[test]
fn a_peek_reads_one_files_uncommitted_lines() {
    use crate::review::{LineKind, Peeked, peek};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    repository(path);
    std::fs::write(path.join("a"), "b\n").unwrap();
    std::fs::write(path.join("new.rs"), "fn main() {}\n").unwrap();
    let input = crate::pull_request::Input {
        checkout: Some(path.to_str().unwrap().into()),
        repo_key: None,
        branch: "main".into(),
    };
    let Peeked::Lines { lines, words, .. } = peek(&input, "a", 40).unwrap() else {
        panic!("expected lines");
    };
    let kinds: Vec<_> = lines.iter().map(|line| line.kind).collect();
    assert_eq!(kinds, [LineKind::Hunk, LineKind::Removed, LineKind::Added]);
    assert_eq!(words.len(), 3);
    // An untracked file is wholly added.
    let Peeked::Lines { lines, .. } = peek(&input, "new.rs", 40).unwrap() else {
        panic!("expected lines");
    };
    let kinds: Vec<_> = lines.iter().map(|line| line.kind).collect();
    assert_eq!(kinds, [LineKind::Hunk, LineKind::Added]);
    assert_eq!(peek(&input, "missing", 40).unwrap(), Peeked::Gone);
    // The branch must still be the one the checkout is on.
    let moved = crate::pull_request::Input {
        branch: "other".into(),
        ..input
    };
    assert!(peek(&moved, "a", 40).is_err());
}
