//! How a strip's "+" looks: the plus, or the mark of the command
//! `plus_command` runs, which turns back into the plus while Alt is held.

use super::HerdrWindow;
use crate::{browser::Slot, plus_command::ALT_KEY};
use gpui::{prelude::*, *};

/// The plus and a command's mark.
const ICON: f32 = 14.;

impl HerdrWindow {
    /// The command's mark, or the plus without one or while Alt is held.
    pub(super) fn plus_icon(&self, slot: Slot) -> AnyElement {
        // Quiet like the unselected tabs beside it.
        let color = self.theme.muted;
        match self.plus_command() {
            Some(command) => div()
                .debug_selector(move || slot.selector("new-tab-command-icon"))
                .child(command.icon().element(ICON, color))
                .into_any_element(),
            None => svg()
                .path("icons/plus.svg")
                .debug_selector(move || slot.selector("new-tab-icon"))
                .size(px(ICON))
                .text_color(rgb(color))
                .into_any_element(),
        }
    }

    /// What a click on "+" opens now.
    pub(super) fn plus_label(&self) -> SharedString {
        match self.plus_command() {
            Some(command) => format!("New {} tab", command.program()).into(),
            None => "New tab".into(),
        }
    }

    /// With `plus_command` set, a tooltip saying what a click opens and that
    /// Alt opens a plain terminal tab instead.
    pub(super) fn plus_tooltip(&self, button: Stateful<Div>) -> Stateful<Div> {
        if self.config.plus_command.is_none() {
            return button;
        }
        let text = match self.plus_command() {
            Some(_) => format!("{} · {ALT_KEY}-click for a terminal", self.plus_label()),
            None => "New terminal tab".to_owned(),
        };
        let text = SharedString::from(text);
        let (foreground, surface) = (self.theme.foreground, self.theme.surface);
        button.tooltip(move |_, cx| {
            cx.new(|_| crate::usage::Hint {
                text: text.clone(),
                foreground,
                surface,
            })
            .into()
        })
    }
}
