//! The tab strip's "+" running a command. With `plus_command = "claude"` a
//! click on "+" opens a terminal tab and types that line into its shell, and
//! "+" shows the command's mark instead of the plus. Holding Alt (Option on
//! macOS) turns it back into a plain "+" for as long as the key is down, and
//! a click then opens a plain terminal tab. The new-tab shortcut always opens
//! a plain terminal tab.
//!
//! The endpoint API has no method that starts a command, so this follows the
//! closed-agent reopen: `tab.create` through the connection's script slot,
//! then the line typed into the shell of the pane it made. The line is the
//! user's own config, typed as written, so it runs in whatever shell the
//! host gives panes, here or on an SSH host.

mod error;

use crate::{HerdrWindow, NavigationTarget, browser::GroupId, icons::AgentIcon, window::Flash};
use gpui::{Context, Modifiers, ModifiersChangedEvent, Window};
use herdr_client::Method;
use serde::Deserialize;
use serde_json::json;
use std::time::{Duration, Instant};

pub use error::Error;

/// The longest command the config may hold.
const MAX_COMMAND_BYTES: usize = 1024;
/// How long a start may wait for the daemon to make its tab.
const DEADLINE: Duration = Duration::from_secs(15);

/// The `plus_command` setting: one line, typed into the new tab's shell.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct PlusCommand(String);

impl TryFrom<String> for PlusCommand {
    type Error = crate::Error;

    fn try_from(command: String) -> crate::Result<Self> {
        let command = command.trim();
        // A newline or escape would run or edit more than the one line.
        if command.is_empty()
            || command.len() > MAX_COMMAND_BYTES
            || command.chars().any(char::is_control)
        {
            return Err(Error::Command.into());
        }
        Ok(Self(command.to_owned()))
    }
}

impl PlusCommand {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// The program the line runs: its first word, without a directory.
    pub(crate) fn program(&self) -> &str {
        let first = self.0.split_whitespace().next().unwrap_or_default();
        first.rsplit(['/', '\\']).next().unwrap_or(first)
    }

    /// The mark "+" shows: the agent's when the program is one Herdr
    /// knows, else the generic agent mark.
    pub(crate) fn icon(&self) -> AgentIcon {
        AgentIcon::from_identity(Some(self.program()))
    }
}

/// What the "+" buttons track between frames.
#[derive(Debug, Default)]
pub(crate) struct PlusTab {
    /// Whether Alt is down, so "+" offers a plain terminal tab.
    alt: bool,
    starting: Option<Starting>,
}

/// A command whose tab is being made, fenced by the selection, connection,
/// and daemon boot it was asked of, like a new chat.
#[derive(Debug)]
struct Starting {
    boot: String,
    deadline: Instant,
    endpoint: (u64, u64),
    endpoint_id: String,
    line: String,
    request: String,
}

/// The name of the key that turns "+" back into a plain terminal tab.
pub(crate) const ALT_KEY: &str = if cfg!(target_os = "macos") {
    "⌥"
} else {
    "Alt"
};

impl HerdrWindow {
    /// The command a click on "+" runs now: none without the setting, or
    /// while Alt is held in this window.
    pub(crate) fn plus_command(&self) -> Option<&PlusCommand> {
        self.config
            .plus_command
            .as_ref()
            .filter(|_| !(self.plus_tab.alt && self.active))
    }

    /// Follows Alt, redrawing "+" when it changes what it opens.
    pub(crate) fn plus_modifiers(
        &mut self,
        event: &ModifiersChangedEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.track_plus_alt(&event.modifiers) {
            cx.notify();
        }
    }

    /// Records whether Alt is down; true when "+" should redraw.
    pub(crate) fn track_plus_alt(&mut self, modifiers: &Modifiers) -> bool {
        let changed = self.plus_tab.alt != modifiers.alt;
        self.plus_tab.alt = modifiers.alt;
        changed && self.config.plus_command.is_some()
    }

    /// A click on `group`'s "+": the command's tab, or a plain terminal tab
    /// with Alt held or without the setting.
    pub(crate) fn click_plus(
        &mut self,
        group: GroupId,
        alt: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let command = self.config.plus_command.clone().filter(|_| !alt);
        let Some(command) = command else {
            self.expect_new_tab_in(group);
            return self.command(crate::controls::Command::Tab, window, cx);
        };
        match self.start_plus_command(&command) {
            Ok(()) => self.expect_new_tab_in(group),
            Err(error) => self.flash_plus_error(error, cx),
        }
        cx.notify();
    }

    /// Opens a tab in the focused pane's workspace and folder for `command`.
    fn start_plus_command(&mut self, command: &PlusCommand) -> Result<(), Error> {
        if self.script_slot_busy() {
            return Err(Error::SlotBusy);
        }
        let snapshot = self
            .live
            .snapshot
            .as_deref()
            .filter(|_| self.live.status.is_connected())
            .ok_or(Error::NotConnected)?;
        let pane = snapshot
            .focused_pane_id
            .as_deref()
            .and_then(|id| snapshot.panes.iter().find(|pane| pane.pane_id == id));
        let workspace = pane
            .map(|pane| pane.workspace_id.clone())
            .or_else(|| snapshot.focused_workspace_id.clone())
            .ok_or(Error::NoWorkspace)?;
        // No label: Herdr keeps a label given here as one the user chose,
        // so a tab-naming plugin would never name the tab after its work.
        let mut params = json!({
            "workspace_id": workspace,
            "focus": true,
        });
        if let Some(cwd) = pane.and_then(|pane| pane.foreground_cwd.as_ref().or(pane.cwd.as_ref()))
        {
            params["cwd"] = json!(cwd);
        }
        let boot = snapshot.boot_id.clone();
        let endpoint = &self.endpoints[self.selected_endpoint];
        let request = endpoint
            .connection
            .request_script(&boot, Method::TabCreate, params)
            .map_err(|_| Error::NotConnected)?;
        self.plus_tab.starting = Some(Starting {
            boot,
            deadline: Instant::now() + DEADLINE,
            endpoint: (self.selection_epoch, endpoint.generation),
            endpoint_id: endpoint.id.clone(),
            line: command.as_str().to_owned(),
            request,
        });
        Ok(())
    }

    /// Whether a command's tab holds the script slot.
    pub(crate) fn plus_command_starting(&self) -> bool {
        self.plus_tab.starting.is_some()
    }

    /// Runs every tick: once the tab exists, types the command and shows
    /// the tab.
    pub(crate) fn poll_plus_command(&mut self, cx: &mut Context<Self>) {
        let Some(starting) = &self.plus_tab.starting else {
            return;
        };
        let current = starting.endpoint
            == (
                self.selection_epoch,
                self.endpoints[self.selected_endpoint].generation,
            )
            && self.live.status.is_connected()
            && self
                .live
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.boot_id == starting.boot);
        if !current {
            self.plus_tab.starting = None;
            return self.flash_plus_error(crate::Error::StaleConnection, cx);
        }
        if Instant::now() >= starting.deadline {
            self.plus_tab.starting = None;
            return self.flash_plus_error(Error::Timeout, cx);
        }
        let request = starting.request.clone();
        let Some(result) = self.take_script_response(&request) else {
            return;
        };
        let Some(starting) = self.plus_tab.starting.take() else {
            return;
        };
        let typed = result
            .map_err(|error| Error::Request(error).into())
            .and_then(|response| {
                crate::worktree_scripts::created_tab(&response).map_err(|error| match error {
                    crate::Error::WorktreeScripts(crate::worktree_scripts::Error::Response) => {
                        Error::Response.into()
                    }
                    error => error,
                })
            })
            .and_then(|(tab, pane)| {
                self.type_script_line(&starting.boot, &pane, &starting.line)?;
                Ok(tab)
            });
        match typed {
            Ok(tab) => {
                self.navigate_endpoint(&starting.endpoint_id, NavigationTarget::Tab(&tab), cx);
                cx.notify();
            }
            Err(error) => self.flash_plus_error(error, cx),
        }
    }

    fn flash_plus_error(&mut self, error: impl std::fmt::Display, cx: &mut Context<Self>) {
        self.show_flash(
            Flash::warning(format!("The new tab did not start: {error}")),
            cx,
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
