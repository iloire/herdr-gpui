//! Each file of the diff drawn as its own card.
use super::window;
use crate::review::diff::Diff;
use crate::review::view::Loaded;
use gpui::px;

fn draw(cx: &mut gpui::VisualTestContext) {
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
}

fn two_files() -> Loaded {
    Loaded::of(Diff::parse(concat!(
        "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n",
        "@@ -1,2 +1,2 @@\n fn a() {}\n-fn b() {}\n+fn b() { todo!() }\n",
        "diff --git a/src/c.rs b/src/c.rs\n--- a/src/c.rs\n+++ b/src/c.rs\n",
        "@@ -1 +1 @@\n-fn c() {}\n+fn c() { 1 }\n",
    )))
}

#[gpui::test]
fn each_file_is_a_card_apart_from_the_next(cx: &mut gpui::TestAppContext) {
    let (view, cx) = window(cx, None);
    cx.update(|window, cx| view.update(cx, |view, cx| view.seed_review(two_files(), window, cx)));
    draw(cx);
    draw(cx);
    let review = cx.debug_bounds("review").unwrap();
    let first = cx.debug_bounds("review-header-0").unwrap();
    let second = cx.debug_bounds("review-header-1").unwrap();
    // The first file's last line, its added `fn b`.
    let last = cx.debug_bounds("review-line-0-3").unwrap();
    assert!(
        second.top() - last.bottom() >= px(8.),
        "a gap parts the files: {last:?} {second:?}"
    );
    assert!(
        first.left() - review.left() >= px(12.) && review.right() - first.right() >= px(12.),
        "cards sit in from the edges: {first:?} {review:?}"
    );
    assert_eq!(first.left(), second.left(), "cards line up");
}
