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
    /// The folder new tabs start in, as the daemon names it.
    pub folder: String,
    /// The checkout the Changes reader found the workspace in, by path.
    pub checkout: Option<String>,
    pub tabs: usize,
    pub host: Option<String>,
    pub branch: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    /// `None` when no read of this host's files was made.
    pub dirty: Option<Dirty>,
    pub agent: Option<Agent>,
    /// The workspace's place in its host's sidebar, which orders the list.
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

impl Card {
    /// The folder's own name, which titles its card: the page lists folders,
    /// and a workspace label can be anything. The label when the daemon
    /// names no folder.
    pub(crate) fn title(&self) -> &str {
        Path::new(&self.folder)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&self.name)
    }

    /// The workspace's label, when it names the folder something other than
    /// its own name, as a second workspace in one folder does.
    pub(crate) fn alias(&self) -> Option<&str> {
        Some(self.name.as_str()).filter(|name| *name != self.title())
    }

    /// Whether the folder's row needs a branch line: not on the usual
    /// default branch level with its upstream, where it would only repeat
    /// what every such folder says. The branch is in the status bar too.
    pub(crate) fn shows_branch(&self) -> bool {
        let default = matches!(self.branch.as_deref(), None | Some("main" | "master"));
        !default || self.ahead > 0 || self.behind > 0
    }

    /// The folder's path as the user reads it: `~` for this computer's home,
    /// in full on another host, whose home is not this one's.
    pub(crate) fn shown_folder(&self) -> String {
        if self.host.is_some() {
            self.folder.clone()
        } else {
            crate::folder::home_relative(&self.folder)
        }
    }

    /// Whether nothing in the folder asks for a look: its agent, if any,
    /// is idle (finished and seen), and nothing waits to be committed or
    /// pushed. The list dims such a row.
    pub(crate) fn quiet(&self) -> bool {
        let idle = self
            .agent
            .as_ref()
            .is_none_or(|agent| agent.state == State::Idle);
        let dirty = self.dirty.is_some_and(|dirty| dirty.files > 0);
        idle && !dirty && self.ahead == 0
    }

    /// Whether the folder's agent waits on the user.
    pub(crate) fn waiting(&self) -> bool {
        self.agent
            .as_ref()
            .is_some_and(|agent| agent.state.needs_you())
    }
}

/// Every host's workspaces as cards, in host and sidebar order: the order
/// the user arranges, by dragging here or in the sidebar.
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
                    folder: workspace.new_workspace_cwd.clone(),
                    checkout: checkout.map(|checkout| checkout.path.clone()),
                    tabs: host
                        .snapshot
                        .tabs
                        .iter()
                        .filter(|tab| tab.workspace_id == workspace.workspace_id)
                        .count(),
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
    cards.sort_by_key(|card| (card.endpoint_index, card.number));
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

/// The rows of `workspace`'s agents on `endpoint`, most pressing first.
pub(crate) fn rows_of<'a>(rows: &'a [Row], endpoint: &str, workspace: &str) -> Vec<&'a Row> {
    let mut mine: Vec<&Row> = rows
        .iter()
        .filter(|row| {
            matches!(&row.open, Open::Pane { endpoint: host, workspace: id, .. }
                if host == endpoint && id == workspace)
        })
        .collect();
    mine.sort_by_key(|row| (row.state, std::cmp::Reverse(row.since)));
    mine
}

/// The workspace's most pressing agent, the one that entered its state last
/// among equals.
fn agent_of(rows: &[Row], endpoint: &str, workspace: &str) -> Option<Agent> {
    let mine = rows_of(rows, endpoint, workspace);
    let lead = mine.first()?;
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
            waiting: cards.iter().filter(|card| card.waiting()).count(),
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
    // FNV-1a: stable across runs, unlike the standard hasher. Its low bits
    // barely mix, so the remainder alone put most names on two slots;
    // Murmur3's finalizer spreads them first.
    let mut hash = name.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85eb_ca6b);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xc2b2_ae35);
    hash ^= hash >> 16;
    1 + (hash % 6) as usize
}
