use super::hue;
use super::page::{bounds, endpoint, fixture};

/// Each row carries its folder's color down its left edge, inside the row.
#[gpui::test]
fn every_folder_row_has_an_edge_bar_at_its_left(cx: &mut gpui::TestAppContext) {
    let (view, cx) = fixture(cx);
    let host = endpoint(&view, cx);
    for workspace in ["w0", "w1"] {
        let row = bounds(cx, &format!("folder-{host}-{workspace}")).expect("row");
        let bar = bounds(cx, &format!("folder-bar-{host}-{workspace}")).expect("bar");
        assert_eq!(bar.origin.x, row.origin.x, "{workspace}");
        assert!(bar.size.width < gpui::px(4.), "{workspace}");
        assert!(bar.origin.y > row.origin.y && bar.bottom() < row.bottom());
    }
}

/// Names spread over every slot. The bare FNV remainder's parity is only the
/// parity of a name's odd bytes, so names alike in it, as notes, loops,
/// herdr-gpui, and wind-hunter are, all fell on two slots.
#[test]
fn hues_spread_across_the_palette_for_names_of_one_parity() {
    let mut counts = [0_usize; 7];
    let names = (0..1200)
        .map(|index| format!("project-{index}"))
        .filter(|name| name.bytes().filter(|byte| byte % 2 == 1).count() % 2 == 0);
    for name in names {
        counts[hue(&name)] += 1;
    }
    let total: usize = counts.iter().sum();
    for (slot, count) in counts.iter().enumerate().skip(1) {
        assert!(*count * 6 > total / 2, "slot {slot}: {counts:?}");
    }
}
