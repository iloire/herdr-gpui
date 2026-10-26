#![allow(clippy::unwrap_used)]
use super::*;
use crate::changes::tests::{Repo, deadline};

#[test]
fn branch_records_read_the_current_mark_counts_and_subjects_with_separators() {
    let text = [
        "*\u{1f}main\u{1f}200\u{1f}0 0\u{1f}feat: a\u{1f}b",
        " \u{1f}agent/x\u{1f}100\u{1f}3 2\u{1f}wip",
        " \u{1f}old\u{1f}50\u{1f}\u{1f}no counts",
        "malformed",
    ]
    .join("\n");
    let branches = parse_branches(&text);
    assert_eq!(branches.len(), 3);
    assert!(branches[0].current);
    assert_eq!(branches[0].subject, "feat: a\u{1f}b");
    assert!(branches[0].merged());
    assert_eq!(branches[1].ahead_behind, Some((3, 2)));
    assert!(!branches[1].merged());
    assert_eq!(branches[2].ahead_behind, None);
    assert!(!branches[2].merged());
}

#[test]
fn reflog_selectors_give_their_time() {
    assert_eq!(
        parse_reflog_time("origin/main@{1730000000}\n"),
        Some(1_730_000_000)
    );
    assert_eq!(parse_reflog_time("origin/main@{0}x"), None);
    assert_eq!(parse_reflog_time(""), None);
}

#[test]
fn activity_counts_days_back_from_now_and_ranks_files() {
    let now = 100 * DAY;
    let text = format!(
        "\u{1e}{}\n\na.rs\nb.rs\n\u{1e}{}\n\na.rs\n\u{1e}{}\n\nold.rs\n\u{1e}{}\n",
        now - 10,
        now - DAY - 10,
        now - DAYS as i64 * DAY - 10,
        now + 50,
    );
    let (days, hot) = parse_activity(&text, now);
    assert_eq!(days.len(), DAYS);
    // A commit from the future (clock skew) counts as today.
    assert_eq!(days[DAYS - 1], 2);
    assert_eq!(days[DAYS - 2], 1);
    assert_eq!(days.iter().sum::<u32>(), 3);
    assert_eq!(hot[0].path, "a.rs");
    assert_eq!(hot[0].commits, 2);
    assert_eq!(hot.len(), 3);
}

#[test]
fn a_real_repository_reads_branches_tag_activity_and_hot_files() {
    let repo = Repo::new();
    repo.write("a", "a\n");
    repo.git(&["add", "a"]);
    repo.git(&["commit", "-qm", "first"]);
    repo.git(&["tag", "v1.0.0"]);
    repo.git(&["remote", "add", "origin", "."]);
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    repo.git(&["switch", "-q", "-c", "agent/task"]);
    for subject in ["one", "two"] {
        repo.write("a", subject);
        repo.git(&["commit", "-qam", subject]);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let overview = read(
        &mut Session::local(),
        repo.path().to_str().unwrap(),
        now,
        deadline(),
        &|| false,
    )
    .unwrap();
    assert_eq!(overview.base.as_deref(), Some("origin/main"));
    let task = overview
        .branches
        .iter()
        .find(|branch| branch.name == "agent/task")
        .unwrap();
    assert!(task.current);
    assert_eq!(task.ahead_behind, Some((2, 0)));
    let main = overview
        .branches
        .iter()
        .find(|branch| branch.name == "main")
        .unwrap();
    assert!(main.merged());
    assert_eq!(
        overview.release,
        Some(Release {
            tag: "v1.0.0".into(),
            time: overview.release.as_ref().unwrap().time,
            since: 2
        })
    );
    assert!(overview.base_moved.is_some());
    assert_eq!(overview.days[DAYS - 1], 3);
    assert_eq!(overview.hot[0].path, "a");
}

#[test]
fn a_repository_without_remote_or_tags_still_lists_its_branches() {
    let repo = Repo::new();
    repo.write("a", "a\n");
    repo.git(&["add", "a"]);
    repo.git(&["commit", "-qm", "first"]);
    let overview = read(
        &mut Session::local(),
        repo.path().to_str().unwrap(),
        0,
        deadline(),
        &|| false,
    )
    .unwrap();
    assert_eq!(overview.base, None);
    assert_eq!(overview.release, None);
    assert_eq!(overview.branches.len(), 1);
    assert_eq!(overview.branches[0].ahead_behind, None);
}
