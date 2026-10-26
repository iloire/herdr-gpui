//! A note opened with nothing typed holds nothing: picking other changes
//! beside a folder's Changes is not refused for it.
use super::*;

#[gpui::test]
fn only_words_typed_in_a_note_hold_the_changes(cx: &mut gpui::TestAppContext) {
    let (view, cx) = window(cx, None);
    cx.update(|window, cx| view.update(cx, |view, cx| view.seed_review(changes(), window, cx)));
    let holds = |view: &Entity<HerdrWindow>, cx: &mut gpui::VisualTestContext| {
        view.read_with(cx, |view, cx| view.reviews[&the(view)].holds_notes(cx))
    };
    // A click on a file's header opens a note with nothing in it.
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.begin_review_note(the(view), RowId::Header(0), window, cx)
        })
    });
    assert!(!holds(&view, cx), "an empty note holds nothing");
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            let input = view.reviews[&the(view)].input.clone();
            input.update(cx, |input, cx| input.set_text_selected("rename this", cx));
        })
    });
    assert!(holds(&view, cx), "words typed in it are kept");
    note(&view, cx, line(1), "and this");
    assert!(holds(&view, cx), "a note waiting to be sent is kept");
}
