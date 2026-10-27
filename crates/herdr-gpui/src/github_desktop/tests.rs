#![allow(clippy::unwrap_used)]
use super::*;

#[test]
fn the_system_app_comes_before_the_users_own() {
    let temp = tempfile::tempdir().unwrap();
    let (system, user) = (temp.path().join("system"), temp.path().join("user"));
    std::fs::create_dir_all(user.join(BUNDLE)).unwrap();
    assert_eq!(
        find([Some(system.as_path()), Some(user.as_path())]),
        Some(user.join(BUNDLE))
    );
    std::fs::create_dir_all(system.join(BUNDLE)).unwrap();
    assert_eq!(
        find([Some(system.as_path()), None, Some(user.as_path())]),
        Some(system.join(BUNDLE))
    );
    assert_eq!(find([None, Some(temp.path())]), None);
}
