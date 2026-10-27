//! Tabs marked as pending: read, but with something still owed there.
//!
//! The mark is a leading [`MARK`] on Herdr's shared tab label, so every client
//! shows it and it outlives this app. herdr-auto-title strips the same prefix
//! when it judges whether a tab was renamed and keeps it when it names one, so
//! the two must agree on it.
//!
//! The mark clears itself once the tab is dealt with: when a line is submitted
//! in it from this app, or when its agent starts working, which the daemon
//! reports whichever client the input came from.

use crate::HerdrWindow;
use gpui::Context;
use herdr_client::{
    ClientHandle, Method,
    protocol::{
        AgentStatus, ClientKeyCode, ClientKeyKind, ClientPaneInputEvent, ClientShellSnapshot,
        ClientShellTab,
    },
};
use serde_json::{Value, json};
use std::collections::HashMap;

/// U+25C6 BLACK DIAMOND and a space. herdr-auto-title has the same constant.
pub(crate) const MARK: &str = "\u{25c6} ";

pub(crate) fn is_marked(label: &str) -> bool {
    label.starts_with(MARK)
}

/// The label without its mark, as the tab is named underneath it.
pub(crate) fn unmarked(label: &str) -> &str {
    label.strip_prefix(MARK).unwrap_or(label)
}

/// The `tab.rename` parameters that add the mark to `tab`, or take it off.
/// Not trimmed, unlike a typed rename, so the mark keeps its space.
pub(crate) fn toggle_params(tab: &str, label: &str) -> Value {
    let label = if is_marked(label) {
        unmarked(label).to_owned()
    } else {
        format!("{MARK}{label}")
    };
    json!({"tab_id": tab, "label": label})
}

/// Whether `event` submits a line: a plain Enter press. Shift-Enter only
/// adds a line to an agent's prompt.
pub(crate) fn submits(event: &ClientPaneInputEvent) -> bool {
    matches!(
        event,
        ClientPaneInputEvent::Key {
            code: ClientKeyCode::Enter,
            kind: ClientKeyKind::Press,
            modifiers: 0,
            ..
        }
    )
}

/// Each tab's agent status at the last snapshot, to see a tab start working.
#[derive(Debug, Default)]
pub(crate) struct Watch {
    boot: String,
    statuses: HashMap<String, AgentStatus>,
}

impl Watch {
    /// The marked tabs whose agent started working since the last snapshot.
    /// Only a change counts: a tab first seen, or every tab of a new boot,
    /// is recorded as it is, so a tab marked while working keeps its mark.
    pub(crate) fn started<'a>(
        &mut self,
        snapshot: &'a ClientShellSnapshot,
    ) -> Vec<&'a ClientShellTab> {
        if self.boot != snapshot.boot_id {
            self.boot.clone_from(&snapshot.boot_id);
            self.statuses.clear();
        }
        let previous = std::mem::take(&mut self.statuses);
        let mut started = Vec::new();
        for tab in &snapshot.tabs {
            let before = previous.get(&tab.tab_id).copied();
            if tab.agent_status == AgentStatus::Working
                && before.is_some_and(|before| before != AgentStatus::Working)
                && is_marked(&tab.label)
            {
                started.push(tab);
            }
            self.statuses.insert(tab.tab_id.clone(), tab.agent_status);
        }
        started
    }
}

/// Asks Herdr to take the mark off `tab`. Nothing to send when it has none.
pub(crate) fn clear(
    handle: &ClientHandle,
    boot: &str,
    tab: &ClientShellTab,
) -> herdr_client::Result<()> {
    if is_marked(&tab.label) {
        handle.request(
            boot,
            Method::TabRename,
            toggle_params(&tab.tab_id, &tab.label),
        )?;
    }
    Ok(())
}

impl HerdrWindow {
    /// Clears the mark of the tab holding `pane` after a line was submitted
    /// there, as answering in the tab deals with what was pending.
    pub(crate) fn clear_pending_for_pane(&mut self, pane: &str, cx: &mut Context<Self>) {
        let (Some(handle), Some(snapshot)) = (
            &self.endpoints[self.selected_endpoint].connection.handle,
            &self.live.snapshot,
        ) else {
            return;
        };
        let Some(tab) = snapshot
            .panes
            .iter()
            .find(|p| p.pane_id == pane)
            .and_then(|p| snapshot.tabs.iter().find(|t| t.tab_id == p.tab_id))
        else {
            return;
        };
        if let Err(error) = clear(handle, &snapshot.boot_id, tab) {
            self.local_error = Some(format!("Pending mark not cleared: {error}"));
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests;
