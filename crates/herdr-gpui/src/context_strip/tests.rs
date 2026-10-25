#![allow(clippy::unwrap_used)]

use super::{
    ContextStrip, INTERVAL, Target,
    render::{pending_worktrees, repo_path},
};
use crate::{
    HerdrWindow,
    sidebar::layout_tests::{fixture_window, full_draw},
};
use gpui::{Bounds, Pixels, TestAppContext, px, size};
use herdr_vcs::{
    changes::{Checkout, FileChange, Kind, Unmerged},
    context::{Context, State, Stopped},
};
use std::time::Instant;

fn target(directory: &str) -> Target {
    Target {
        directory: directory.into(),
        local: true,
        repo_key: None,
    }
}

fn checkout(path: &str, branch: &str) -> Checkout {
    Checkout {
        path: path.into(),
        common_dir: "/code/app/.git".into(),
        branch: Some(branch.into()),
        ..Checkout::default()
    }
}

fn context(checkout: Checkout) -> Context {
    Context {
        checkout,
        state: State::default(),
        worktrees: Vec::new(),
    }
}

fn dirty(mut checkout: Checkout) -> Checkout {
    checkout.files = vec![FileChange {
        path: "src/main.rs".into(),
        kind: Kind::Modified,
        additions: Some(12),
        deletions: Some(3),
    }];
    checkout.total_files = 1;
    checkout
}

#[test]
fn worktrees_with_work_not_on_the_default_branch_are_pending() {
    let clean = checkout("/code/clean", "clean");
    let behind = Checkout {
        behind: 4,
        ..checkout("/code/behind", "behind")
    };
    let unpushed = Checkout {
        ahead: 1,
        ..checkout("/code/unpushed", "unpushed")
    };
    let unmerged = Checkout {
        unmerged: Some(Unmerged {
            base: "origin/main".into(),
            count: 2,
        }),
        ..checkout("/code/unmerged", "unmerged")
    };
    let edited = dirty(checkout("/code/edited", "edited"));
    let mut read = context(checkout("/code/app", "main"));
    read.worktrees = vec![clean, behind, unpushed, unmerged, edited];
    assert_eq!(pending_worktrees(&read), 3);
}

#[test]
fn the_path_names_the_checkout_and_where_the_pane_is_inside_it() {
    assert_eq!(
        repo_path("/code/app", "/code/app/crates/core"),
        ("/code/app".to_owned(), "/crates/core".to_owned())
    );
    assert_eq!(
        repo_path("/code/app", "/code/app"),
        ("/code/app".to_owned(), String::new())
    );
    // A folder reached through a link Git resolved shows the checkout alone.
    assert_eq!(
        repo_path("/private/code/app", "/code/app/src"),
        ("/private/code/app".to_owned(), String::new())
    );
}

#[test]
fn moving_focus_to_another_folder_forgets_the_last_read() {
    let mut strip = ContextStrip::default();
    strip.seed(
        target("/code/app"),
        Ok(context(checkout("/code/app", "main"))),
    );
    assert!(strip.read.is_some());

    // The same folder in another workspace keeps what was read.
    let keyed = Target {
        repo_key: Some("/code/app/.git".into()),
        ..target("/code/app")
    };
    assert!(strip.aim(Some(keyed.clone())));
    assert!(strip.read.is_some());
    assert!(!strip.aim(Some(keyed)));

    assert!(strip.aim(Some(target("/code/other"))));
    assert!(strip.read.is_none());
    assert!(strip.due(Instant::now()));
}

#[test]
fn a_read_finishing_after_focus_moved_is_dropped() {
    let mut strip = ContextStrip::default();
    strip.aim(Some(target("/code/other")));
    strip.install(
        "/code/app",
        Ok(context(checkout("/code/app", "main"))),
        Instant::now() + INTERVAL,
    );
    assert!(strip.read.is_none());
    // The new folder is read at the next tick instead of waiting.
    assert!(strip.due(Instant::now()));
}

fn seed(view: &gpui::Entity<HerdrWindow>, read: Context, cx: &mut gpui::VisualTestContext) {
    view.update(cx, |view, cx| {
        view.context_strip.seed(target("/code/app"), Ok(read));
        cx.notify();
    });
}

fn bounds(selector: &'static str, cx: &mut gpui::VisualTestContext) -> Option<Bounds<Pixels>> {
    cx.update(|window, cx| full_draw(window, cx).clear(cx));
    cx.debug_bounds(selector)
}

/// The strip takes a row between the terminal and the status bar, and
/// turning it off hands the row back to the terminal.
#[gpui::test]
fn the_strip_sits_between_the_terminal_and_the_status_bar(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.simulate_resize(size(px(1000.), px(600.)));
    seed(&view, context(checkout("/code/app", "main")), cx);

    let strip = bounds("context-strip", cx).unwrap();
    let terminal = bounds("terminal", cx).unwrap();
    let bar = bounds("connection-status", cx).unwrap();
    assert_eq!(terminal.bottom(), strip.top());
    assert_eq!(strip.bottom(), bar.top());
    let path = bounds("context-path", cx).unwrap();
    let branch = bounds("context-branch", cx).unwrap();
    assert!(path.right() <= branch.left());

    view.update(cx, |view, cx| {
        view.config.show_context_strip = false;
        cx.notify();
    });
    assert!(bounds("context-strip", cx).is_none());
    let grown = bounds("terminal", cx).unwrap();
    assert_eq!(grown.bottom(), bar.top());
    assert_eq!(grown.size.height, terminal.size.height + strip.size.height);
}

#[gpui::test]
fn problems_get_their_own_segments_and_quiet_ones_stay_away(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.simulate_resize(size(px(1400.), px(600.)));
    seed(&view, context(checkout("/code/app", "main")), cx);
    for quiet in [
        "context-stopped",
        "context-crypt",
        "context-stash",
        "context-worktrees",
    ] {
        assert!(bounds(quiet, cx).is_none(), "{quiet}");
    }
    assert!(bounds("context-changes", cx).is_some());

    let mut read = context(dirty(checkout("/code/app", "fix/a-very-long-branch-name")));
    read.state = State {
        stopped: Some(Stopped::Rebase { step: Some((3, 7)) }),
        stashes: 2,
        crypt_locked: true,
    };
    read.worktrees = vec![dirty(checkout("/code/app-side", "side"))];
    seed(&view, read, cx);
    for shown in [
        "context-branch",
        "context-stopped",
        "context-changes",
        "context-stash",
        "context-worktrees",
        "context-crypt",
    ] {
        assert!(bounds(shown, cx).is_some(), "{shown}");
    }
    let strip = bounds("context-strip", cx).unwrap();
    let crypt = bounds("context-crypt", cx).unwrap();
    assert!(crypt.right() <= strip.right());
}

/// On another host nothing is read, and the strip says why.
#[gpui::test]
fn another_hosts_folder_shows_no_git_segments(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    view.update(cx, |view, cx| {
        view.context_strip.aim(Some(Target {
            local: false,
            ..target("/srv/app")
        }));
        cx.notify();
    });
    assert!(bounds("context-strip", cx).is_some());
    assert!(bounds("context-branch", cx).is_none());
}
