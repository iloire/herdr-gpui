#![allow(clippy::unwrap_used)]
// Not a glob: the parent's `gpui` imports would shadow `#[test]`.
use super::{DiskWatch, Follow};
use crate::browser::TabId;
use std::time::{Duration, SystemTime};

fn at(seconds: u64) -> Option<SystemTime> {
    Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
}

#[test]
fn a_file_seen_for_the_first_time_only_records() {
    let mut disk = DiskWatch::default();
    let (code, page) = (TabId::test(1), TabId::test(2));
    assert!(
        disk.settle(vec![
            (code, Follow::Read, at(1)),
            (page, Follow::Reload, at(1))
        ])
        .is_empty()
    );
    assert!(
        disk.settle(vec![
            (code, Follow::Read, at(1)),
            (page, Follow::Reload, at(1))
        ])
        .is_empty()
    );
}

#[test]
fn a_moved_time_refreshes_the_tab_its_own_way() {
    let mut disk = DiskWatch::default();
    let (code, page) = (TabId::test(1), TabId::test(2));
    disk.settle(vec![
        (code, Follow::Read, at(1)),
        (page, Follow::Reload, at(1)),
    ]);
    let changed = disk.settle(vec![
        (code, Follow::Read, at(2)),
        (page, Follow::Reload, at(3)),
    ]);
    assert_eq!(changed, [(code, Follow::Read), (page, Follow::Reload)]);
}

#[test]
fn a_file_missing_mid_save_still_refreshes_when_it_is_back() {
    let mut disk = DiskWatch::default();
    let code = TabId::test(1);
    disk.settle(vec![(code, Follow::Read, at(1))]);
    // Renamed into place: for a moment there is nothing to read.
    assert!(disk.settle(vec![(code, Follow::Read, None)]).is_empty());
    assert_eq!(
        disk.settle(vec![(code, Follow::Read, at(2))]),
        [(code, Follow::Read)]
    );
}

#[test]
fn a_closed_tab_is_forgotten() {
    let mut disk = DiskWatch::default();
    let code = TabId::test(1);
    disk.settle(vec![(code, Follow::Read, at(1))]);
    disk.settle(Vec::new());
    assert!(disk.settle(vec![(code, Follow::Read, at(5))]).is_empty());
}
