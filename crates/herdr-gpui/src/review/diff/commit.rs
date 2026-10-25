//! A commit reviewed on its own: its name, checked wherever it comes from,
//! and what its header shows. The diff runs from its first parent, or from
//! the empty tree for a root commit, to the commit itself.
use crate::{notifications::safe_text, pull_request::Input};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Instant};

/// The empty tree's name in SHA-1 and SHA-256 repositories: what a root
/// commit is compared with.
const EMPTY_TREE_SHA1: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
const EMPTY_TREE_SHA256: &str = "6ef19b41225c5369f1c104d45d8d85efa9b057b53b14b4b9b939dd74decc5321";
/// Characters of the subject and author a header keeps.
const MAX_SUBJECT_CHARS: usize = 300;
const MAX_AUTHOR_CHARS: usize = 100;

/// A commit's full object name: 40 lowercase hex digits, or 64 in a SHA-256
/// repository. Saved with the tabs, so it is checked whenever it is read,
/// and nothing Git could take for an option, a range, or a ref.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) struct CommitId(String);

impl TryFrom<String> for CommitId {
    type Error = crate::review::Error;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        let valid = matches!(name.len(), 40 | 64)
            && name
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if !valid {
            return Err(crate::review::Error::InvalidCommit);
        }
        Ok(Self(name))
    }
}

impl From<CommitId> for String {
    fn from(commit: CommitId) -> Self {
        commit.0
    }
}

impl CommitId {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// The name abbreviated as Git does by default.
    pub(crate) fn short(&self) -> &str {
        &self.0[..7]
    }
}

/// What a commit review's header says about the commit, as display text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Summary {
    pub subject: String,
    pub author: String,
}

/// The checkout a commit is read in: the one the review names, while it
/// still belongs to the review's repository. Unlike the working tree's
/// changes, a commit does not need its branch checked out, so a checkout
/// that moved to another branch, or to none, still shows it. Blocking.
pub(super) fn checkout(input: &Input, deadline: Instant) -> crate::Result<String> {
    let Some(directory) = input.checkout.as_deref() else {
        return Ok(crate::pull_request::local_checkout(
            input,
            deadline,
            &|| false,
        )?);
    };
    let refused = |error: crate::pull_request::Error| -> crate::Error {
        herdr_github::Error::from(error).into()
    };
    if !Path::new(directory).is_absolute() {
        return Err(refused(crate::pull_request::Error::AbsolutePath));
    }
    let found = crate::git::git(
        directory,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-common-dir",
        ],
        "find the checkout",
        deadline,
        &|| false,
    )?;
    let mut lines = found.lines();
    let (Some(top), Some(common)) = (lines.next(), lines.next()) else {
        return Err(refused(crate::pull_request::Error::RepositoryMismatch));
    };
    let same = |key: &str| {
        Path::new(common)
            .canonicalize()
            .ok()
            .zip(Path::new(key).canonicalize().ok())
            .is_some_and(|(actual, expected)| actual == expected)
    };
    if input.repo_key.as_deref().is_some_and(|key| !same(key)) {
        return Err(refused(crate::pull_request::Error::RepositoryMismatch));
    }
    Ok(top.to_owned())
}

/// The commit's summary and the revision its diff starts from: its first
/// parent, or the empty tree. Blocking.
pub(super) fn read(
    checkout: &str,
    commit: &CommitId,
    deadline: Instant,
) -> crate::Result<(Summary, String)> {
    let never = || false;
    let shown = crate::git::git(
        checkout,
        &[
            "show",
            "-s",
            "--no-color",
            "--format=%P%x1f%an%x1f%s",
            commit.as_str(),
        ],
        "read the commit",
        deadline,
        &never,
    )?;
    let (parents, summary) = parse_shown(&shown);
    let parent = match parents {
        Some(parent) => parent,
        None => {
            let format = crate::git::git(
                checkout,
                &["rev-parse", "--show-object-format"],
                "read the repository's hash",
                deadline,
                &never,
            )?;
            empty_tree(&format).to_owned()
        }
    };
    Ok((summary, parent))
}

/// `git show --format=%P%x1f%an%x1f%s` as the first parent, if any, and
/// the summary. A merge is compared with its first parent, as `git show
/// --first-parent` would.
pub(super) fn parse_shown(shown: &str) -> (Option<String>, Summary) {
    let mut fields = shown.splitn(3, '\u{1f}');
    let parent = fields
        .next()
        .and_then(|parents| parents.split_whitespace().next())
        .and_then(|parent| CommitId::try_from(parent.to_owned()).ok())
        .map(String::from);
    let author = safe_text(fields.next().unwrap_or_default(), MAX_AUTHOR_CHARS);
    let subject = safe_text(fields.next().unwrap_or_default(), MAX_SUBJECT_CHARS);
    (parent, Summary { subject, author })
}

/// The empty tree in a repository whose objects are named by `format`.
pub(super) fn empty_tree(format: &str) -> &'static str {
    if format.trim() == "sha256" {
        EMPTY_TREE_SHA256
    } else {
        EMPTY_TREE_SHA1
    }
}
