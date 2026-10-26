use super::{
    page::{bounds, click, draw, endpoint, fixture},
    repo, shown,
};
use crate::{HerdrWindow, changes::Read};
use gpui::{Entity, VisualTestContext};
use herdr_vcs::changes::overview::{Branch, DAYS, Hot, Overview, Release};

/// This machine's daemon read w0 in `/tmp` and, beside it, a worktree
/// with unmerged work that w1 is open in.
fn with_checkouts(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.live.local_daemon_peer = true;
            let mut worktree = shown("/tmp/agent", &["w1"], 2, 1);
            worktree.checkout.common_dir = "/tmp/.git".into();
            view.changes.read_for_test(Read {
                repos: vec![repo(vec![shown("/tmp", &["w0"], 0, 0), worktree])],
                failed: Vec::new(),
            });
            cx.notify();
        })
    });
    draw(cx);
}

fn overview() -> Overview {
    let branch = |name: &str, current, ahead_behind| Branch {
        name: name.into(),
        subject: format!("work on {name}"),
        time: 1,
        current,
        ahead_behind,
    };
    let mut days = vec![0; DAYS];
    days[DAYS - 1] = 3;
    Overview {
        branches: vec![
            branch("main", true, Some((0, 0))),
            branch("agent/task", false, Some((2, 1))),
        ],
        base: Some("origin/main".into()),
        base_moved: Some(1),
        release: Some(Release {
            tag: "v1.0.0".into(),
            time: 1,
            since: 4,
        }),
        days,
        hot: vec![Hot {
            path: "src/main.rs".into(),
            commits: 3,
        }],
    }
}

#[gpui::test]
fn home_lists_every_card_and_no_buttons_that_repeat_the_sidebar(cx: &mut gpui::TestAppContext) {
    let (_, cx) = fixture(cx);
    for card in [
        "folder-home-uncommitted",
        "folder-home-agents",
        "folder-home-pull-requests",
        "folder-home-issues",
        "folder-home-commits",
        "folder-home-branches",
        "folder-home-sync",
        "folder-home-worktrees",
        "folder-home-activity",
        "folder-home-ports",
    ] {
        assert!(bounds(cx, card).is_some(), "{card}");
    }
    assert!(bounds(cx, "folder-open-terminals").is_none());
    assert!(bounds(cx, "folder-open-files").is_none());
}

#[gpui::test]
fn the_overview_fills_branches_and_other_worktrees_open_their_folder(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    with_checkouts(&view, cx);
    assert!(bounds(cx, "folder-home-branch-0").is_none(), "not read yet");
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.folders.overview.seed("/tmp", overview());
            cx.notify();
        })
    });
    draw(cx);
    assert!(bounds(cx, "folder-home-branch-0").is_some());
    assert!(bounds(cx, "folder-home-branch-1").is_some());
    // The overview of another checkout is not this folder's.
    assert!(bounds(cx, "folder-home-worktree-1").is_none(), "one other");
    click(cx, "folder-home-worktree-0");
    let picked = cx.update(|_, cx| view.read(cx).folders.selected.clone());
    assert_eq!(picked, Some((host, "w1".into())));
    assert!(
        bounds(cx, "folder-home-branch-0").is_none(),
        "w1's checkout has no overview read"
    );
}
