use super::*;
use crate::config::preferences::Preference;
use crate::settings_window::controls::preferences::PreferenceIo;
use std::sync::{Arc, Mutex};

/// Both switches on the Sidebar page save the opposite of what they show,
/// each to its own key.
#[gpui::test]
fn show_pull_requests_and_issues_save_the_opposite(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    let edits = Arc::new(Mutex::new(Vec::new()));
    // What the file holds, so the reload after a save reads the write back.
    let file = Arc::new(Mutex::new((true, true)));
    let (captured, written, read) = (edits.clone(), file.clone(), file.clone());
    view.update(cx, |view, cx| {
        view.controls.preference_io = Some(PreferenceIo {
            write: Arc::new(move |edit| {
                match edit {
                    Preference::SidebarPullRequests(shown) => written.lock().unwrap().0 = shown,
                    Preference::SidebarIssues(shown) => written.lock().unwrap().1 = shown,
                    _ => {}
                }
                captured.lock().unwrap().push(edit);
                Ok(())
            }),
            load: Arc::new(move || {
                let mut loaded = skill_load()?;
                let (pull_requests, issues) = *read.lock().unwrap();
                loaded.config.sidebar_style.pull_requests = pull_requests;
                loaded.config.sidebar_style.issues = issues;
                Ok(loaded)
            }),
        });
        view.section = Section::Sidebar;
        cx.notify();
    });
    cx.simulate_resize(size(px(960.), px(2200.)));
    for id in ["sidebar-pull-requests-toggle", "sidebar-issues-toggle"] {
        let preference = |shown| match id {
            "sidebar-pull-requests-toggle" => Preference::SidebarPullRequests(shown),
            _ => Preference::SidebarIssues(shown),
        };
        for shown in [true, false] {
            cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
            let switch = cx.debug_bounds(id).unwrap();
            cx.simulate_click(switch.center(), Default::default());
            cx.run_until_parked();
            assert_eq!(
                edits.lock().unwrap().pop(),
                Some(preference(!shown)),
                "{id}"
            );
        }
    }
    view.update(cx, |view, _| {
        assert!(view.config.sidebar_style.pull_requests);
        assert!(view.config.sidebar_style.issues);
    });
}
