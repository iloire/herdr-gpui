//! Reviewing the changes an agent made: the focused checkout's uncommitted
//! diff, notes on its lines, and sending them back to the agent the way page
//! annotations go (see `agent_notes`).
mod diff;
mod error;
pub(crate) mod highlight;
mod notes;
mod view;

pub(crate) use diff::{CommitId, Kind as LineKind, Peeked, lockfile, peek, push_clean};
pub(crate) use error::Error;
pub(crate) use view::{Review, agent_in, line_look, styled_code};
