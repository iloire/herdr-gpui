use super::super::menu::{Action, FilesMenu};
use std::path::Path;

fn labels(path: &str, folder: bool) -> Vec<&'static str> {
    let menu = FilesMenu::new(Path::new("/repo"), path, folder);
    menu.actions()
        .into_iter()
        .map(|action| menu.label(action))
        .collect()
}

#[test]
fn a_file_opens_and_a_folder_becomes_a_herdr_folder() {
    let file = labels("src/main.rs", false);
    assert_eq!(file[..2], ["Open", "Open in Editor"]);
    assert!(!file.contains(&"Open as New Folder"));
    let folder = labels("src", true);
    assert_eq!(folder[0], "Open as New Folder");
    assert!(!folder.contains(&"Open"));
    for labels in [file, folder] {
        assert!(labels.ends_with(&["Copy Path", "Copy Relative Path"]));
    }
}

#[cfg(target_os = "macos")]
#[test]
fn a_file_s_kind_names_its_actions() {
    assert_eq!(
        labels("README.md", false)[..3],
        ["Open Rendered", "Open as Source", "Open in Editor"]
    );
    assert!(labels("site/index.HTML", false).contains(&"Open in Browser"));
    assert!(labels("logo.png", false).contains(&"Open with Default App"));
    assert!(labels("src", true).contains(&"Open in Finder"));
    assert!(!labels("src", true).contains(&"Reveal in Finder"));
    assert!(labels("logo.png", false).contains(&"Reveal in Finder"));
}

#[test]
fn every_action_is_listed_once() {
    let menu = FilesMenu::new(Path::new("/repo"), "README.md", false);
    let actions = menu.actions();
    let unique: std::collections::HashSet<Action> = actions.iter().copied().collect();
    assert_eq!(unique.len(), actions.len());
}
