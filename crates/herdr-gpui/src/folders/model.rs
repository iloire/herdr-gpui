//! What the Folders page shows, gathered from the window's prepared state
//! before drawing: one card per open workspace on every connected host,
//! with the Git state its daemon reports, the uncommitted files the Changes
//! reader found in it, and its most pressing agent from the Overview's rows.

use crate::{
    changes::Repo,
    overview::{Open, Row, State},
};
use herdr_client::protocol::ClientShellSnapshot;
use std::path::Path;

/// One connected host's workspaces, as the window holds them.
pub(crate) struct Host<'a> {
    pub index: usize,
    pub id: &'a str,
    /// The device's label, shown on cards of a host other than this one.
    pub label: Option<&'a str>,
    pub snapshot: &'a ClientShellSnapshot,
    /// What the Changes reader found, for this machine's own daemon only:
    /// its workspace ids are the ones that reader was given.
    pub repos: Option<&'a [Repo]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Card {
    pub endpoint_index: usize,
    pub endpoint: String,
    pub workspace: String,
    pub name: String,
    pub host: Option<String>,
    pub branch: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    /// `None` when no read of this host's files was made.
    pub dirty: Option<Dirty>,
    pub agent: Option<Agent>,
    /// The workspace's place in its host's sidebar, which orders ties.
    number: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Dirty {
    pub files: usize,
    pub added: u64,
    pub removed: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Agent {
    pub state: State,
    pub identity: Option<String>,
    pub title: String,
    /// How many agents the workspace runs, this one included.
    pub count: usize,
}

/// What a card asks of the user, most pressing first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rank {
    NeedsYou,
    Dirty,
    Unpushed,
    Working,
    Quiet,
}

impl Card {
    pub(crate) fn rank(&self) -> Rank {
        let state = self.agent.as_ref().map(|agent| agent.state);
        if state.is_some_and(State::needs_you) {
            Rank::NeedsYou
        } else if self.dirty.is_some_and(|dirty| dirty.files > 0) {
            Rank::Dirty
        } else if self.ahead > 0 {
            Rank::Unpushed
        } else if state == Some(State::Working) {
            Rank::Working
        } else {
            Rank::Quiet
        }
    }

    /// Whether Needs me lists the card: it waits on the user, or holds work
    /// not yet committed or pushed.
    pub(crate) fn needs_me(&self) -> bool {
        self.rank() <= Rank::Unpushed
    }
}

/// Every host's workspaces as cards, most pressing first, then in host and
/// sidebar order.
pub(crate) fn cards<'a>(hosts: impl IntoIterator<Item = Host<'a>>, rows: &[Row]) -> Vec<Card> {
    let mut cards: Vec<Card> = hosts
        .into_iter()
        .flat_map(|host| {
            host.snapshot.workspaces.iter().map(move |workspace| {
                let checkout = host.repos.and_then(|repos| {
                    checkout_of(repos, &workspace.workspace_id, &workspace.new_workspace_cwd)
                });
                let (ahead, behind) = match checkout {
                    Some(checkout) => (checkout.ahead as usize, checkout.behind as usize),
                    None => workspace.git_ahead_behind.unwrap_or_default(),
                };
                Card {
                    endpoint_index: host.index,
                    endpoint: host.id.to_owned(),
                    workspace: workspace.workspace_id.clone(),
                    name: workspace.label.clone(),
                    host: host.label.map(str::to_owned),
                    branch: checkout
                        .and_then(|checkout| checkout.branch.clone())
                        .or_else(|| workspace.branch.clone()),
                    ahead,
                    behind,
                    // With a read of this machine, a workspace outside Git
                    // has nothing uncommitted.
                    dirty: host.repos.map(|_| {
                        checkout.map_or_else(Dirty::default, |checkout| {
                            let (added, removed) = checkout.lines();
                            Dirty {
                                files: checkout.total_files,
                                added,
                                removed,
                            }
                        })
                    }),
                    agent: agent_of(rows, host.id, &workspace.workspace_id),
                    number: workspace.number,
                }
            })
        })
        .collect();
    cards.sort_by_key(|card| (card.rank(), card.endpoint_index, card.number));
    cards
}

/// The checkout a workspace is in: the one holding its folder, else the
/// first a pane of it was found in.
fn checkout_of<'a>(
    repos: &'a [Repo],
    workspace: &str,
    folder: &str,
) -> Option<&'a herdr_vcs::changes::Checkout> {
    let mut found = repos
        .iter()
        .flat_map(|repo| &repo.checkouts)
        .filter(|shown| shown.workspaces.iter().any(|open| open.id == workspace))
        .map(|shown| &shown.checkout);
    let first = found.next()?;
    let holds =
        |checkout: &herdr_vcs::changes::Checkout| Path::new(folder).starts_with(&checkout.path);
    if holds(first) {
        return Some(first);
    }
    Some(found.find(|checkout| holds(checkout)).unwrap_or(first))
}

/// The workspace's most pressing agent, the one that entered its state last
/// among equals.
fn agent_of(rows: &[Row], endpoint: &str, workspace: &str) -> Option<Agent> {
    let mine: Vec<&Row> = rows
        .iter()
        .filter(|row| {
            matches!(&row.open, Open::Pane { endpoint: host, workspace: id, .. }
                if host == endpoint && id == workspace)
        })
        .collect();
    let lead = mine
        .iter()
        .min_by_key(|row| (row.state, std::cmp::Reverse(row.since)))?;
    Some(Agent {
        state: lead.state,
        identity: lead.identity.clone(),
        title: lead.title.clone(),
        count: mine.len(),
    })
}

/// The page's summary: cards waiting on the user, dirty, and unpushed, and
/// how many hosts they are on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Summary {
    pub waiting: usize,
    pub dirty: usize,
    pub unpushed: usize,
    pub hosts: usize,
}

impl Summary {
    pub(crate) fn of(cards: &[Card]) -> Self {
        let mut hosts: Vec<usize> = cards.iter().map(|card| card.endpoint_index).collect();
        hosts.sort_unstable();
        hosts.dedup();
        Self {
            waiting: cards
                .iter()
                .filter(|card| card.rank() == Rank::NeedsYou)
                .count(),
            dirty: cards
                .iter()
                .filter(|card| card.dirty.is_some_and(|dirty| dirty.files > 0))
                .count(),
            unpushed: cards.iter().filter(|card| card.ahead > 0).count(),
            hosts: hosts.len(),
        }
    }
}

/// A folder's mark: the first letters of its name's first two words, or
/// its first two letters when it has one word.
pub(crate) fn monogram(name: &str) -> String {
    let words: Vec<&str> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    let letters: String = match words.as_slice() {
        [] => "?".into(),
        [word] => word.chars().take(2).collect(),
        [first, second, ..] => first
            .chars()
            .take(1)
            .chain(second.chars().take(1))
            .collect(),
    };
    letters.to_uppercase()
}

/// The palette slot, red to cyan, a folder's mark is drawn in: the same for
/// a name on every launch and every host.
pub(crate) fn hue(name: &str) -> usize {
    // FNV-1a: stable across runs, unlike the standard hasher.
    let hash = name.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    1 + (hash % 6) as usize
}
