//! A folder's Home: its title, then cards for what it holds and where it
//! stands, what may need the user first. Its uncommitted files and recent
//! commits (`work`) open its Changes; its pull requests and issues
//! (`github`) open their pages; its branches, worktrees, sync, and activity
//! come from the repository overview (`repo`, read by `read`); an agent
//! opens its pane in its Terminals; a port opens in a browser tab.

mod github;
mod read;
mod repo;
mod work;

pub(crate) use read::Reads;

use repo::{home_activity, home_branches, home_sync};

use super::{
    model::{self, Card},
    view::folder_header,
};
use crate::{
    HerdrWindow,
    config::{corners, mix},
    devices_overview::Look,
    icons::AgentIcon,
};
use gpui::{prelude::*, *};

/// Rows a card lists before it says how many more.
const LISTED: usize = 8;

impl HerdrWindow {
    pub(super) fn render_folder_home(
        &self,
        look: &Look,
        card: &Card,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let checkout = card.checkout.as_ref().and_then(|path| {
            self.changes
                .repos()
                .iter()
                .flat_map(|repo| &repo.checkouts)
                .map(|shown| &shown.checkout)
                .find(|checkout| checkout.path == *path)
        });
        let overview = card
            .checkout
            .as_deref()
            .and_then(|path| self.folders.overview.of(path));
        let github = self.folder_github(card, cx);
        let pair = |left: Stateful<Div>, right: Stateful<Div>| {
            div().flex().gap_4().min_w_0().child(left).child(right)
        };
        div()
            .id("folder-home")
            .debug_selector(|| "folder-home".into())
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.folders.body_scroll)
            .p_5()
            .flex()
            .flex_col()
            .gap_4()
            .child(folder_header(look, card))
            .child(pair(
                self.home_uncommitted(look, card, checkout, cx),
                self.home_agents(look, card, cx),
            ))
            .child(pair(
                self.home_pull_requests(look, card, github.as_ref(), cx),
                self.home_issues(look, card, github.as_ref(), cx),
            ))
            .child(pair(
                self.home_commits(look, card, checkout, cx),
                home_branches(look, card, overview),
            ))
            .child(pair(
                home_sync(look, card, checkout, overview, github.as_ref()),
                self.home_worktrees(look, card, cx),
            ))
            .child(pair(
                home_activity(look, card, overview),
                self.home_ports(look, card, cx),
            ))
    }

    fn home_agents(&self, look: &Look, card: &Card, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = look.theme;
        let rows = self.overview_rows(cx);
        let mine = model::rows_of(&rows, &card.endpoint, |id| card.holds(id));
        let count = (!mine.is_empty()).then(|| mine.len().to_string());
        let mut list = div().flex().flex_col().gap_1();
        if mine.is_empty() {
            list = list.child(quiet(look, "No agent running"));
        }
        for (index, row) in mine.into_iter().enumerate() {
            let (clicked, folder) = (row.clone(), card.clone());
            list = list.child(
                div()
                    .id(("folder-home-agent", index))
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .px_1()
                    .py_0p5()
                    .rounded(px(corners::SMALL))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(theme.active)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_folder_agent(&folder, &clicked, window, cx);
                    }))
                    .child(crate::icons::icon(
                        AgentIcon::from_identity(row.identity.as_deref()).path(),
                        14.,
                        theme.foreground,
                    ))
                    .child(look.dot(row.state.status()))
                    .child(div().flex_1().min_w_0().truncate().child(row.title.clone()))
                    .child(
                        div()
                            .flex_none()
                            .max_w(px(140.))
                            .truncate()
                            .text_size(look.small())
                            .text_color(rgb(theme.muted))
                            .child(row.detail.clone()),
                    ),
            );
        }
        panel(look, "folder-home-agents", "Agents", count).child(list)
    }

    fn home_ports(&self, look: &Look, card: &Card, cx: &mut Context<Self>) -> Stateful<Div> {
        let body = match self.folder_ports(look, card, cx) {
            Some(ports) => ports.into_any_element(),
            None => quiet(look, "No ports open").into_any_element(),
        };
        panel(look, "folder-home-ports", "Running here", None).child(body)
    }

    /// The ports listening in the folder's workspace, as chips that open
    /// them, when the port monitor found any: on its Home and its row.
    pub(super) fn folder_ports(
        &self,
        look: &Look,
        card: &Card,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        self.endpoints
            .get(card.endpoint_index)
            .and_then(|endpoint| crate::listening_ports::Daemon::of(&endpoint.connection.target))
            .filter(|_| self.config.show_listening_ports)
            .and_then(|daemon| self.listening_ports.get(&daemon, &card.workspace))
            .map(|listed| {
                crate::listening_ports::chips(
                    listed,
                    (&card.endpoint, &card.workspace),
                    &self.tunnels,
                    look.theme,
                    look.ui.size - 1.,
                    cx,
                )
            })
    }
}

/// A card of the Home: a small title and count over its body.
pub(super) fn panel(
    look: &Look,
    id: &'static str,
    title: &'static str,
    count: Option<String>,
) -> Stateful<Div> {
    let theme = look.theme;
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap_2()
        .p_3()
        .rounded(px(corners::PANEL))
        .bg(rgb(theme.surface))
        .border_1()
        .border_color(rgb(mix(theme.surface, theme.foreground, 8)))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_size(look.small())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme.subtext()))
                .child(title.to_uppercase())
                .when_some(count, |head, count| {
                    head.child(div().text_color(rgb(theme.muted)).child(count))
                }),
        )
}

pub(super) fn quiet(look: &Look, text: impl Into<SharedString>) -> Div {
    div().text_color(rgb(look.theme.muted)).child(text.into())
}

pub(super) fn more(look: &Look, total: usize) -> Option<Div> {
    (total > LISTED).then(|| {
        div()
            .text_size(look.small())
            .text_color(rgb(look.theme.muted))
            .child(format!("and {} more", total - LISTED))
    })
}
