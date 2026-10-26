use super::{cards, snapshot};
use crate::folders::model::Host;

fn shows_branch(branch: &str, sync: (usize, usize)) -> bool {
    let shown = snapshot(&[("w1", "x", "/repo/x", branch, sync)]);
    let host = Host {
        index: 0,
        id: "local",
        label: None,
        snapshot: &shown,
        repos: None,
    };
    cards([host], &[]).remove(0).shows_branch()
}

/// A folder on its default branch, level with its upstream, needs no
/// branch line; any other branch, or commits to push or pull, does.
#[test]
fn the_branch_line_shows_only_when_it_says_something() {
    assert!(!shows_branch("main", (0, 0)));
    assert!(!shows_branch("master", (0, 0)));
    assert!(shows_branch("main", (2, 0)));
    assert!(shows_branch("main", (0, 1)));
    assert!(shows_branch("feature/x", (0, 0)));
}
