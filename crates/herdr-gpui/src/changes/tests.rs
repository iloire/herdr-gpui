#![allow(clippy::unwrap_used)]

use super::{Changes, Rank, Read, Workspace, group, rank, repo_name, section::age};
use herdr_vcs::changes::{Checkout, FileChange, Kind};
use std::time::Duration;

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

#[test]
fn a_checkout_ranks_by_what_it_asks_of_the_user() {
    let clean = checkout("/r", "/r/.git");
    assert_eq!(rank(&clean), Rank::Clean);
    let behind = Checkout {
        behind: 3,
        ..clean.clone()
    };
    assert_eq!(rank(&behind), Rank::Behind);
    let unpushed = Checkout {
        ahead: 1,
        ..behind.clone()
    };
    assert_eq!(rank(&unpushed), Rank::Unpushed);
    assert_eq!(rank(&dirty(unpushed)), Rank::Dirty);
}

#[test]
fn worktrees_group_under_their_repository_and_what_needs_work_leads() {
    let read = group(vec![
        (
            workspace("1"),
            Ok(checkout("/code/zeta", "/code/zeta/.git")),
        ),
        (workspace("2"), Ok(checkout("/code/app", "/code/app/.git"))),
        (
            workspace("3"),
            Ok(dirty(checkout("/code/app-feature", "/code/app/.git"))),
        ),
        (
            workspace("4"),
            Ok(Checkout {
                ahead: 2,
                ..checkout("/code/Beta", "/code/Beta/.git")
            }),
        ),
        // A second workspace in a checkout already read joins it.
        (
            workspace("5"),
            Ok(checkout("/code/zeta", "/code/zeta/.git")),
        ),
    ]);
    let names: Vec<_> = read.repos.iter().map(|repo| repo.name.as_str()).collect();
    assert_eq!(names, ["app", "Beta", "zeta"]);
    let app: Vec<_> = read.repos[0]
        .checkouts
        .iter()
        .map(|shown| shown.checkout.path.as_str())
        .collect();
    assert_eq!(app, ["/code/app-feature", "/code/app"]);
    assert_eq!(
        read.repos[2].checkouts[0].workspaces,
        [workspace("1"), workspace("5")]
    );
    assert!(read.failed.is_empty());
}

#[test]
fn a_failed_read_is_kept_beside_the_repositories() {
    let error = herdr_vcs::Error::from(herdr_vcs::git::Error::NoCheckout);
    let read = group(vec![
        (workspace("1"), Err(error)),
        (workspace("2"), Ok(checkout("/r", "/r/.git"))),
    ]);
    assert_eq!(read.repos.len(), 1);
    assert_eq!(read.failed.len(), 1);
    assert_eq!(read.failed[0].workspace, workspace("1"));
}

#[test]
fn the_bar_counts_checkouts_with_uncommitted_work() {
    let mut changes = Changes::default();
    assert_eq!(changes.dirty(), 0);
    changes.read_for_test(group(vec![
        (workspace("1"), Ok(dirty(checkout("/a", "/a/.git")))),
        (workspace("2"), Ok(dirty(checkout("/a-2", "/a/.git")))),
        (workspace("3"), Ok(checkout("/b", "/b/.git"))),
    ]));
    assert_eq!(changes.dirty(), 2);
    changes.read_for_test(Read::default());
    assert_eq!(changes.dirty(), 0);
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
