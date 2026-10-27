use super::*;

fn window_with_label<'a>(
    cx: &'a mut TestAppContext,
    label: &'static str,
) -> (Entity<HerdrWindow>, &'a mut VisualTestContext) {
    cx.add_window_view(move |window, cx| {
        let mut view = crate::sidebar::layout_tests::fixture_window(window, cx);
        let mut snapshot: ClientShellSnapshot = serde_json::from_str(include_str!(
            "../../../../herdr-protocol/tests/fixtures/endpoint-snapshot-v1.json"
        ))
        .unwrap();
        snapshot.tabs[0].label = label.into();
        view.live.snapshot = Some(Arc::new(snapshot));
        view
    })
}

fn offered(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) -> Vec<Action> {
    cx.update(|window, cx| {
        view.update(cx, |v, cx| {
            let id = v.live.snapshot.as_ref().unwrap().tabs[0].tab_id.clone();
            v.open_tab_menu(&id, None, Point::default(), window, cx);
            v.menu.tab.as_ref().unwrap().actions()
        })
    })
}

#[gpui::test]
fn an_unmarked_tab_offers_to_mark_it_pending(cx: &mut TestAppContext) {
    let (view, cx) = window_with_label(cx, "claude");
    let actions = offered(&view, cx);
    assert!(actions.contains(&Action::MarkPending));
    assert!(!actions.contains(&Action::ClearPending));
    // It sits after Rename, as the other edit of the tab's name.
    let rename = actions.iter().position(|a| *a == Action::Rename).unwrap();
    assert_eq!(actions[rename + 1], Action::MarkPending);
}

#[gpui::test]
fn a_marked_tab_offers_to_clear_it(cx: &mut TestAppContext) {
    let (view, cx) = window_with_label(cx, "\u{25c6} claude");
    let actions = offered(&view, cx);
    assert!(actions.contains(&Action::ClearPending));
    assert!(!actions.contains(&Action::MarkPending));
}

#[gpui::test]
fn marking_without_a_connection_closes_the_menu_and_says_why(cx: &mut TestAppContext) {
    let (view, cx) = window_with_label(cx, "claude");
    offered(&view, cx);
    cx.update(|window, cx| {
        view.update(cx, |v, cx| {
            v.activate_tab_menu(Action::MarkPending, window, cx);
            assert!(v.menu.page.is_none());
            assert!(
                v.local_error
                    .as_deref()
                    .unwrap()
                    .starts_with("Pending mark not changed")
            );
        })
    });
}
