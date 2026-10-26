//! A line's syntax colours and composed text, cut into the runs it is drawn
//! in. The colours come from the file as last coloured, which may be a
//! keystroke behind the text: a span past the line's end or off a
//! character boundary is dropped, and the next colouring puts it right.

use super::display::Shown;
use crate::review::highlight::{Span, Token};
use std::ops::Range;

/// One stretch of a drawn line: its drawn byte range, its token if
/// coloured, and whether it is composed (underlined).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Segment {
    pub(crate) range: Range<usize>,
    pub(crate) token: Option<Token>,
    pub(crate) marked: bool,
}

/// `line` (as drawn in `shown`) cut at every edge of `spans` and of
/// `marked`, both in the line's own byte offsets.
pub(crate) fn segments(
    line: &str,
    shown: &Shown,
    spans: &[Span],
    marked: Option<Range<usize>>,
) -> Vec<Segment> {
    let valid = |at: usize| at <= line.len() && line.is_char_boundary(at);
    let spans: Vec<&Span> = spans
        .iter()
        .filter(|span| span.start < span.end && valid(span.start) && valid(span.end))
        .collect();
    let mut edges: Vec<usize> = vec![0, line.len()];
    edges.extend(spans.iter().flat_map(|span| [span.start, span.end]));
    edges.extend(marked.iter().flat_map(|marked| [marked.start, marked.end]));
    edges.retain(|at| valid(*at));
    edges.sort_unstable();
    edges.dedup();
    edges
        .windows(2)
        .filter_map(|edge| {
            let (start, end) = (edge[0], edge[1]);
            let range = shown.drawn(start)..shown.drawn(end);
            (!range.is_empty()).then(|| Segment {
                range,
                token: spans
                    .iter()
                    .find(|span| span.start <= start && end <= span.end)
                    .map(|span| span.token),
                marked: marked
                    .as_ref()
                    .is_some_and(|marked| marked.start <= start && end <= marked.end),
            })
        })
        .collect()
}
