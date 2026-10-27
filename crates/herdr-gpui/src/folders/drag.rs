//! Reordering folders by dragging their rows, as the sidebar reorders its
//! workspaces (`sidebar::reorder`): holding a press on a row, or moving it a
//! few pixels, lifts the row; the other folders of its host shift to open the
//! gap it would land in; releasing sends the move to that host's daemon. The
//! daemon owns the order, so the sidebar and every other client follow, and
//! the preview holds until its next snapshot reorders the list.
//!
//! A folder moves among its own host's folders, with every workspace open
//! in it gathered behind its first.

use super::model::Card;
use crate::{
    HerdrWindow,
    reorder::{LIFT_DELAY, LIFT_DISTANCE, SETTLE_TIMEOUT, SLIDE, Slide, slot_for},
    sidebar::MoveBlock,
    state::LiveState,
};
use gpui::{Context, Pixels, Point, ScrollHandle, Task};
use std::{cell::RefCell, collections::HashMap, time::Instant};

/// A press on a folder row that may become a reorder.
pub(crate) struct FolderDrag {
    endpoint: usize,
    boot: String,
    /// The folder's first workspace, which stands for its row.
    workspace: String,
    /// Every workspace in the folder, moved as one block.
    members: Vec<String>,
    origin: Point<Pixels>,
    pointer: Point<Pixels>,
    pub(super) lifted: bool,
    /// The gap under the pointer among the host's folders, `None` over the
    /// carried row's own place.
    pub(super) slot: Option<usize>,
    /// The daemon's order when the drop was sent, kept so the preview lasts
    /// exactly until its answer.
    dropped: Option<Vec<String>>,
    /// Each shifted row's slide, by workspace. Render reads and advances them.
    slides: RefCell<HashMap<String, Slide>>,
    /// Lifts the row once the press has rested, then ends a dropped preview
    /// the daemon never answered. Dropping the drag cancels it.
    _timer: Task<()>,
}

/// What render prepares for one drag: the rows of the host's folders, by
/// their place among the list's items, in order, with the carried one's index.
#[derive(Clone, Debug, Default)]
pub(super) struct Rows {
    pub(super) items: Vec<usize>,
    pub(super) ids: Vec<String>,
    pub(super) dragged: usize,
}

impl FolderDrag {
    fn offset(&self) -> Pixels {
        self.pointer.y - self.origin.y
    }

    fn floating(&self) -> bool {
        self.lifted && self.dropped.is_none()
    }

    /// The host's folders as the list shows them, while a lifted drag
    /// previews its drop: while it floats, and after the drop until the
    /// daemon's order replaces the one it was sent against.
    pub(super) fn rows(&self, cards: &[Card], live: Option<&LiveState>) -> Option<Rows> {
        if !self.lifted {
            return None;
        }
        let order = live?.snapshot.as_ref()?.workspaces.iter();
        if self.dropped.as_ref().is_some_and(|dropped| {
            !dropped
                .iter()
                .map(String::as_str)
                .eq(order.map(|workspace| workspace.workspace_id.as_str()))
        }) {
            return None;
        }
        let (items, ids): (Vec<usize>, Vec<String>) = cards
            .iter()
            .enumerate()
            .filter(|(_, card)| card.endpoint_index == self.endpoint)
            .map(|(item, card)| (item, card.workspace.clone()))
            .unzip();
        let dragged = ids.iter().position(|id| *id == self.workspace)?;
        Some(Rows {
            items,
            ids,
            dragged,
        })
    }

    /// How far each of the host's rows paints from its resting place, and
    /// whether any is still sliding. `heights` are the rows' laid-out
    /// heights, gap included.
    pub(super) fn shifts(&self, rows: &Rows, heights: &[f32], now: Instant) -> (Vec<f32>, bool) {
        let slot = self.slot.unwrap_or(rows.dragged);
        let targets = crate::reorder::preview(heights, rows.dragged, slot);
        let mut slides = self.slides.borrow_mut();
        let mut sliding = false;
        let shifts = rows
            .ids
            .iter()
            .zip(targets)
            .enumerate()
            .map(|(index, (id, to))| {
                if index == rows.dragged && self.floating() {
                    // Pinned at the pointer, so once dropped it slides from
                    // there rather than from where it was picked up.
                    let at = f32::from(self.offset());
                    slides.insert(
                        id.clone(),
                        Slide {
                            from: at,
                            to: at,
                            start: now,
                        },
                    );
                    return at;
                }
                let slide = slides.entry(id.clone()).or_insert(Slide {
                    from: 0.,
                    to: 0.,
                    start: now,
                });
                *slide = slide.toward(to, now, SLIDE);
                sliding |= slide.progress(now, SLIDE) < 1.;
                slide.at(now, SLIDE)
            })
            .collect();
        (shifts, sliding)
    }

    /// Whether `workspace` is the row carried under the pointer.
    pub(super) fn carries(&self, endpoint: usize, workspace: &str) -> bool {
        self.floating() && self.endpoint == endpoint && self.workspace == workspace
    }
}

/// The gap the carried row points to, from where the host's rows rest.
fn slot(rows: &Rows, shifts: &[f32], scroll: &ScrollHandle, lift: Pixels) -> Option<usize> {
    let offset = scroll.offset().y;
    // Measured where the rows rest, or the list would chase its own preview.
    let spans: Vec<Option<(f32, f32)>> = rows
        .items
        .iter()
        .zip(shifts)
        .map(|(&item, &shift)| {
            let bounds = scroll.bounds_for_item(item)?;
            let top = f32::from(bounds.top() + offset) - shift;
            Some((top, top + f32::from(bounds.size.height)))
        })
        .collect();
    let (top, bottom) = (*spans.get(rows.dragged)?)?;
    let lift = f32::from(lift);
    let slot = slot_for(&spans, rows.dragged, (top + lift, bottom + lift));
    (slot != rows.dragged && slot != rows.dragged + 1).then_some(slot)
}

impl HerdrWindow {
    /// The live state of endpoint `index`, as the window holds it.
    fn endpoint_live(&self, index: usize) -> Option<&LiveState> {
        if index == self.selected_endpoint {
            Some(&self.live)
        } else {
            self.endpoints.get(index).map(|endpoint| &endpoint.live)
        }
    }

    /// The folder drag's rows for this frame, if one is lifted.
    pub(super) fn folder_drag_rows(&self, cards: &[Card]) -> Option<Rows> {
        let drag = self.folders.drag.as_ref()?;
        drag.rows(cards, self.endpoint_live(drag.endpoint))
    }

    /// Arms a reorder for a left press on a folder row.
    pub(super) fn press_folder(
        &mut self,
        card: &Card,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(live) = self.endpoint_live(card.endpoint_index) else {
            return;
        };
        let Some(snapshot) = live.snapshot.as_ref() else {
            return;
        };
        if self.menu.page.is_some() || !live.status.is_connected() || snapshot.workspaces.len() < 2
        {
            return;
        }
        let boot = snapshot.boot_id.clone();
        let lift = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(LIFT_DELAY).await;
            let _ = this.update(cx, |this, cx| this.lift_folder(cx));
        });
        self.folders.drag = Some(FolderDrag {
            endpoint: card.endpoint_index,
            boot,
            workspace: card.workspace.clone(),
            members: card
                .workspaces
                .iter()
                .map(|member| member.id.clone())
                .collect(),
            origin: position,
            pointer: position,
            lifted: false,
            slot: None,
            dropped: None,
            slides: RefCell::default(),
            _timer: lift,
        });
        cx.notify();
    }

    fn lift_folder(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = &mut self.folders.drag
            && !drag.lifted
        {
            drag.lifted = true;
            cx.notify();
        }
    }

    /// Follows the pointer, resolving the gap against the rows render laid
    /// out. Returns whether the move belongs to the drag.
    pub(super) fn move_folder_drag(
        &mut self,
        position: Point<Pixels>,
        held: bool,
        rows: Option<&(Rows, Vec<f32>)>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(drag) = self.folders.drag.as_mut().filter(|d| d.dropped.is_none()) else {
            return false;
        };
        // A release outside the window never reaches the list.
        if !held {
            let lifted = drag.lifted;
            self.folders.drag = None;
            cx.notify();
            return lifted;
        }
        drag.pointer = position;
        let moved = position - drag.origin;
        if !drag.lifted && f32::from(moved.x.abs().max(moved.y.abs())) > LIFT_DISTANCE {
            drag.lifted = true;
        }
        if !drag.lifted {
            return false;
        }
        drag.slot = rows.and_then(|(rows, shifts)| {
            slot(rows, shifts, &self.folders.list_scroll, drag.offset())
        });
        cx.notify();
        true
    }

    /// Ends the drag on release, sending its move if it was lifted over a
    /// gap. Returns whether it had lifted, so the release is not a click.
    pub(super) fn release_folder_drag(
        &mut self,
        rows: Option<&Rows>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(mut drag) = self.folders.drag.take() else {
            return false;
        };
        if !drag.floating() {
            return false;
        }
        cx.notify();
        let (Some(slot), Some(rows)) = (drag.slot, rows) else {
            return true;
        };
        let connection = &self.endpoints[drag.endpoint].connection;
        let Some(live) = self.endpoint_live(drag.endpoint) else {
            return true;
        };
        let (Some(handle), Some(snapshot)) = (&connection.handle, &live.snapshot) else {
            return true;
        };
        if snapshot.boot_id != drag.boot {
            return true;
        }
        let request = MoveBlock::new(drag.members.clone(), rows.ids.get(slot).cloned())
            .reorder(&snapshot.workspaces, live.supports_workspace_move);
        match handle.reorder_workspaces(&drag.boot, &request) {
            Ok(_) => {
                drag.dropped = Some(
                    snapshot
                        .workspaces
                        .iter()
                        .map(|workspace| workspace.workspace_id.clone())
                        .collect(),
                );
                drag._timer = cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(SETTLE_TIMEOUT).await;
                    let _ = this.update(cx, |this, cx| {
                        this.folders.drag = None;
                        cx.notify();
                    });
                });
                self.folders.drag = Some(drag);
            }
            Err(error) => self.local_error = Some(format!("Folder move not sent: {error}")),
        }
        true
    }

    /// Drops a lifted folder where it came from. Returns whether one was.
    pub(crate) fn cancel_folder_drag(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.folders.drag.as_ref().is_some_and(FolderDrag::floating) {
            return false;
        }
        self.folders.drag = None;
        cx.notify();
        true
    }
}
