//! One file's uncommitted lines, read outside a review: the Changes
//! section's quick look under a file. It lists the checkout's changes as a
//! review would, so renames, untracked files and the size limits behave the
//! same, then reads that one file and colours its first lines. Blocking.
use super::{Body, Lines, Request, Scope, emphasis, load::bodies};
use crate::{pull_request::Input, review::highlight};
use std::{ops::Range, sync::Arc};

/// What a quick look at a file found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Peeked {
    /// Its lines, with the syntax colours and changed words of the first
    /// `limit`, by line.
    Lines {
        lines: Arc<Lines>,
        spans: Vec<Vec<highlight::Span>>,
        words: Vec<Vec<Range<u32>>>,
    },
    /// Not text.
    Binary,
    /// Too large to read without opening a review.
    Large,
    /// No longer changed, or not found among the changes.
    Gone,
    /// Could not be read; why, as the user reads it.
    Failed(String),
}

/// `git_path`'s uncommitted lines in the checkout `input` names.
pub(crate) fn peek(input: &Input, git_path: &str, limit: usize) -> crate::Result<Peeked> {
    let loaded = super::load(input, Scope::Uncommitted, None, None, false)?;
    let Some((index, file)) = loaded
        .diff
        .files
        .iter()
        .enumerate()
        .find(|(_, file)| file.git_path() == git_path)
    else {
        return Ok(Peeked::Gone);
    };
    let body = bodies(&loaded.source, &[Request::of(index, file)], false)
        .pop()
        .map(|(_, body)| body);
    Ok(match body {
        Some(Body::Loaded(lines)) => {
            let shown = 0..lines.len().min(limit);
            Peeked::Lines {
                spans: highlight::colour(&file.path, &lines, shown.clone()),
                words: emphasis(&lines, shown),
                lines,
            }
        }
        Some(Body::Binary) => Peeked::Binary,
        Some(Body::Large) => Peeked::Large,
        Some(Body::Failed(why)) => Peeked::Failed(why),
        Some(Body::Pending) | None => Peeked::Gone,
    })
}
