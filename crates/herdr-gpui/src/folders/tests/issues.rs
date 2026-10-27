//! The selected folder lists its repository's open issues under its parts,
//! and the row opens the Issues section narrowed to the folder's project.
use super::page::{bounds, click, draw, fixture};
use crate::{
    HerdrWindow,
    orchestrator::{Item, ItemKey, Provider, SourceKey},
    sidebar::ActivityPanel,
};
use gpui::{Entity, VisualTestContext};
use std::sync::Arc;

fn issue(number: u32, state: &str) -> Item {
    Item {
        key: ItemKey {
            source: SourceKey {
                provider: Provider::Github,
                host: "github.com".into(),
                repository: "penso/app".into(),
            },
            native_id: number.to_string(),
        },
        identifier: format!("#{number}"),
        title: format!("issue {number}"),
        description: None,
        state: state.into(),
        url: None,
        author: None,
        labels: Vec::new(),
        parent_id: None,
        blocked_by: Vec::new(),
        priority: None,
        created_at: None,
        updated_at: None,
        pull_request: None,
        activity: None,
    }
}

/// The local host's repositories listed in the sections' view, each holding
/// `items`.
fn listed(
    view: &Entity<HerdrWindow>,
    root: &std::path::Path,
    items: Vec<Item>,
    cx: &mut VisualTestContext,
) {
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.endpoints[0].connection.target = herdr_client::ConnectTarget::Local;
            view.prepare_orchestrator_projects_for_tests(root, cx);
            if let Some(sections) = view.section_view() {
                sections.update(cx, |sections, _| {
                    for repo in sections.repos_mut_for_tests() {
                        repo.snapshot.items = Arc::new(items.clone());
                    }
                });
            }
            cx.notify();
        })
    });
    draw(cx);
}

#[gpui::test]
fn issues_lists_the_open_ones_and_opens_the_section(cx: &mut gpui::TestAppContext) {
    let repositories = tempfile::tempdir().unwrap();
    for folder in ["worktrees", "other"] {
        gix::init(repositories.path().join(folder)).unwrap();
    }
    let (view, cx) = fixture(cx);
    assert!(
        bounds(cx, "folder-part-issues").is_none(),
        "nothing listed yet"
    );
    listed(
        &view,
        repositories.path(),
        vec![issue(1, "open"), issue(2, "closed"), issue(3, "open")],
        cx,
    );
    let row = bounds(cx, "folder-part-issues").expect("open issues show a row");
    let files = bounds(cx, "folder-part-files").unwrap();
    assert!(row.top() >= files.bottom(), "after the parts");
    let open = cx.update(|_, cx| {
        let view = view.read(cx);
        let cards = view.folder_cards(cx);
        let card = view.selected_folder(&cards).unwrap();
        view.folder_open_issues(card, cx)
    });
    assert_eq!(open, Some(2), "closed ones are not counted");
    click(cx, "folder-part-issues");
    cx.update(|_, cx| {
        let view = view.read(cx);
        assert_eq!(view.activity.panel, ActivityPanel::Issues);
    });
}

#[gpui::test]
fn no_row_without_an_open_issue(cx: &mut gpui::TestAppContext) {
    let repositories = tempfile::tempdir().unwrap();
    for folder in ["worktrees", "other"] {
        gix::init(repositories.path().join(folder)).unwrap();
    }
    let (view, cx) = fixture(cx);
    listed(&view, repositories.path(), vec![issue(1, "closed")], cx);
    assert!(bounds(cx, "folder-part-files").is_some());
    assert!(bounds(cx, "folder-part-issues").is_none());
}
