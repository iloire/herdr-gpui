//! Each file of the diff drawn as a card, so one file's lines never run on
//! into the next: its header is the card's top, its lines the card's
//! sides, its last row the card's bottom, and a gap parts it from the next
//! file. The list keeps one row per item; only each row's frame changes.
//! GPUI clips to rectangles, so the header rounds its own top corners and
//! the last row leaves room under its line for the rounded bottom.

use super::Review;
use crate::config::{Theme, corners};
use gpui::{prelude::*, *};

/// Space either side of the cards.
const GUTTER: f32 = 12.;
/// Space between one file's card and the next.
const GAP: f32 = 6.;
/// From the list's edge to the card's inside: the gutter and its border.
pub(super) const INSET: f32 = GUTTER + 1.;
/// The cards' corners.
pub(super) const RADIUS: f32 = corners::CONTROL;

/// Which part of its file's card a row draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Edges {
    pub top: bool,
    pub bottom: bool,
}

impl Review {
    /// The edges of its card the row at `position` draws: a file's header
    /// is its top, its last row its bottom, and a folded file's header both.
    pub(super) fn edges(&self, position: usize) -> Edges {
        let Some(file) = self.item(position).map(|item| item.file()) else {
            return Edges::default();
        };
        let start = self.starts.get(file).copied();
        let end = self.starts.get(file + 1).copied();
        Edges {
            top: start == Some(position),
            bottom: end == Some(position + 1),
        }
    }
}

/// `row` inside its card, with the edges `edges` names.
pub(super) fn frame(theme: &Theme, row: AnyElement, edges: Edges) -> Div {
    let border = rgb(theme.active);
    div()
        .w_full()
        .px(px(GUTTER))
        .when(edges.top, |slot| slot.pt(px(GAP)))
        .when(edges.bottom, |slot| slot.pb(px(GAP)))
        .child(
            div()
                .w_full()
                .border_l_1()
                .border_r_1()
                .border_color(border)
                .when(edges.top, |card| card.border_t_1().rounded_t(px(RADIUS)))
                .when(edges.bottom, |card| {
                    // Room under the last line keeps its tint off the
                    // rounded corners.
                    card.border_b_1()
                        .rounded_b(px(RADIUS))
                        .when(!edges.top, |card| card.pb(px(RADIUS / 2.)))
                })
                .child(row),
        )
}

/// Where the split divider sits in a list `width` wide: the card's
/// inside, split at `ratio`.
pub(super) fn split_x(width: Pixels, ratio: f32) -> Pixels {
    px(INSET) + (width - px(2. * INSET)).max(px(0.)) * ratio
}

/// The split ratio a divider dragged to `offset` from the list's left
/// edge gives, in a list `width` wide; `None` when the cards have no room.
pub(super) fn split_ratio(offset: Pixels, width: Pixels) -> Option<f32> {
    let inside = f32::from(width) - 2. * INSET;
    (inside > 0.).then(|| (f32::from(offset) - INSET) / inside)
}
