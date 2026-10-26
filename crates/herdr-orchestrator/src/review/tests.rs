#![allow(clippy::unwrap_used)]
use super::*;

fn response(next: Option<&str>) -> Value {
    json!({ "data": { "repository": { "pullRequests": {
        "pageInfo": { "hasNextPage": next.is_some(), "endCursor": next },
        "nodes": [
            {
                "number": 601,
                "reviewDecision": "CHANGES_REQUESTED",
                "reviewRequests": { "nodes": [
                    { "requestedReviewer": { "__typename": "User", "login": "penso" } },
                    { "requestedReviewer": { "__typename": "Team", "slug": "core" } },
                    { "requestedReviewer": { "__typename": "Mannequin" } },
                    null
                ] },
                "commits": { "nodes": [ { "commit": { "statusCheckRollup": { "contexts": { "nodes": [
                    { "__typename": "CheckRun", "name": "macos", "status": "COMPLETED", "conclusion": "SUCCESS" },
                    { "__typename": "CheckRun", "name": "windows", "status": "COMPLETED", "conclusion": "FAILURE" },
                    { "__typename": "CheckRun", "name": "lint", "status": "IN_PROGRESS", "conclusion": null },
                    { "__typename": "CheckRun", "name": "docs", "status": "COMPLETED", "conclusion": "SKIPPED" },
                    { "__typename": "StatusContext", "context": "ci/legacy", "state": "PENDING" }
                ] } } } } ] }
            },
            {
                "number": 602,
                "reviewDecision": null,
                "reviewRequests": { "nodes": [] },
                "commits": { "nodes": [ { "commit": { "statusCheckRollup": null } } ] }
            }
        ]
    } } } })
}

#[test]
fn a_page_reads_reviews_reviewers_and_checks() {
    let page = parse(response(Some("cursor"))).unwrap();
    assert_eq!(page.next.as_deref(), Some("cursor"));
    let first = &page.statuses[&601];
    assert_eq!(first.review, Some(Review::ChangesRequested));
    assert_eq!(first.requested, ["penso", "team/core"]);
    assert!(first.requests("PENSO"));
    let states: Vec<_> = first.checks.iter().map(|check| check.state).collect();
    assert_eq!(
        states,
        [
            CheckState::Passed,
            CheckState::Failed,
            CheckState::Pending,
            CheckState::Skipped,
            CheckState::Pending,
        ]
    );
    // Any failure wins over what is still running.
    assert_eq!(first.checks_state(), Some(CheckState::Failed));
    // No checks and no verdict yet: nothing to say.
    let second = &page.statuses[&602];
    assert_eq!(second.review, None);
    assert_eq!(second.checks_state(), None);
    assert!(parse(response(None)).unwrap().next.is_none());
}

#[test]
fn reading_stops_at_the_last_page_or_the_page_limit() {
    let mut calls = 0;
    let statuses = fetch("penso/herdr-gpui", |_, variables| {
        calls += 1;
        // The second page is asked for with the first one's cursor.
        if calls == 2 {
            assert_eq!(variables["after"], "cursor");
        }
        Ok(response(Some("cursor")))
    })
    .unwrap();
    assert_eq!(calls, PAGES);
    assert_eq!(statuses.pull_requests.len(), 2);
    let mut calls = 0;
    fetch("penso/herdr-gpui", |_, _| {
        calls += 1;
        Ok(response(None))
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert!(matches!(
        fetch("not-a-repository", |_, _| Ok(response(None))),
        Err(Error::RemoteUrl(_))
    ));
}

#[test]
fn the_first_page_reads_the_default_branch_checks_and_later_ones_skip_them() {
    let mut calls = 0;
    let statuses = fetch("penso/herdr-gpui", |_, variables| {
        calls += 1;
        assert_eq!(variables["first"], calls == 1);
        let mut page = response((calls == 1).then_some("cursor"));
        if calls == 1 {
            page["data"]["repository"]["defaultBranchRef"] = json!({
                "name": "main",
                "target": { "statusCheckRollup": { "contexts": { "nodes": [
                    { "__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "SUCCESS" },
                    { "__typename": "CheckRun", "name": "test", "status": "COMPLETED", "conclusion": "FAILURE" }
                ] } } }
            });
        }
        Ok(page)
    })
    .unwrap();
    assert_eq!(calls, 2);
    let main = statuses.default_branch.unwrap();
    assert_eq!(main.branch, "main");
    assert_eq!(main.checks.len(), 2);
    assert_eq!(main.checks_state(), Some(CheckState::Failed));
}

#[test]
fn a_default_branch_without_checks_or_a_shape_not_understood_says_nothing_wrong() {
    let mut page = response(None);
    page["data"]["repository"]["defaultBranchRef"] =
        json!({ "name": "main", "target": { "statusCheckRollup": null } });
    let main = parse(page).unwrap().default_branch.unwrap();
    assert!(main.checks.is_empty());
    assert_eq!(main.checks_state(), None);
    let mut page = response(None);
    page["data"]["repository"]["defaultBranchRef"] = json!({ "name": 5 });
    let page = parse(page).unwrap();
    assert_eq!(page.default_branch, None);
    assert_eq!(page.statuses.len(), 2);
}
