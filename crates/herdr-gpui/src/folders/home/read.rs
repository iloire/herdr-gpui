//! The selected folder's repository overview (`herdr_vcs::changes::overview`)
//! for its Home: read on the background executor while the Home is on
//! screen, again every [`INTERVAL`], and at once when another folder is
//! picked. Only this machine's checkouts are read, as the Changes reader's
//! are; drawing reads what the last read left.

use crate::HerdrWindow;
use gpui::{prelude::*, *};
use herdr_vcs::{changes::overview::Overview, host::Session};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// How often the shown Home reads again.
const INTERVAL: Duration = Duration::from_secs(30);
/// The read's Git children together.
const TIMEOUT: Duration = Duration::from_secs(15);

/// The overview reads' state in a window.
#[derive(Default)]
pub(crate) struct Reads {
    /// The checkout last read, by path, and what it held; `None` when the
    /// read failed.
    last: Option<(String, Option<Arc<Overview>>)>,
    /// When the next read is due.
    next: Option<Instant>,
    task: Option<Task<()>>,
    /// Tells a read in flight to stop when the window goes.
    cancelled: Arc<AtomicBool>,
}

impl Drop for Reads {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Reads {
    /// What the last read of `checkout` found.
    pub(crate) fn of(&self, checkout: &str) -> Option<&Overview> {
        self.last
            .as_ref()
            .filter(|(path, _)| path == checkout)
            .and_then(|(_, overview)| overview.as_deref())
    }

    /// Installs `overview` as a read of `checkout` would, for tests.
    #[cfg(test)]
    pub(crate) fn seed(&mut self, checkout: &str, overview: Overview) {
        self.last = Some((checkout.to_owned(), Some(Arc::new(overview))));
        self.next = Some(Instant::now() + INTERVAL);
    }

    /// Whether `checkout` should be read now.
    fn due(&self, checkout: &str, now: Instant) -> bool {
        let other = self.last.as_ref().is_none_or(|(path, _)| path != checkout);
        self.task.is_none() && (other || self.next.is_none_or(|next| now >= next))
    }
}

impl HerdrWindow {
    /// Runs every tick: reads the shown Home's checkout when it is due.
    pub(crate) fn poll_folder_overview(&mut self, cx: &mut Context<Self>) {
        if !self.folders_on_screen() || self.folders.part != super::super::Part::Home {
            return;
        }
        let cards = self.folder_cards(cx);
        let Some(checkout) = self
            .selected_folder(&cards)
            .and_then(|card| card.checkout.clone())
        else {
            return;
        };
        if !self.folders.overview.due(&checkout, Instant::now()) {
            return;
        }
        let cancelled = self.folders.overview.cancelled.clone();
        let path = checkout.clone();
        let work = cx.background_executor().spawn(async move {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_secs().cast_signed());
            herdr_vcs::changes::overview::read(
                &mut Session::local(),
                &path,
                now,
                Instant::now() + TIMEOUT,
                &|| cancelled.load(Ordering::Relaxed),
            )
        });
        self.folders.overview.task = Some(cx.spawn(async move |this, cx| {
            let read = work.await;
            let _ = this.update(cx, |this, cx| {
                let overview = match read {
                    Ok(overview) => Some(Arc::new(overview)),
                    Err(error) => {
                        tracing::warn!(
                            category = "folder_overview",
                            checkout = %checkout,
                            %error,
                            "Cannot read a folder's overview"
                        );
                        None
                    }
                };
                let reads = &mut this.folders.overview;
                reads.last = Some((checkout, overview));
                reads.next = Some(Instant::now() + INTERVAL);
                reads.task = None;
                cx.notify();
            });
        }));
    }
}
