//! What a checkout holds right now, for the Changes section: its branch and
//! how far it is from its upstream, the files with uncommitted changes and
//! their line counts, and its latest commits. A few read-only Git children
//! per checkout, run by the caller off the UI thread; nothing here contacts
//! a remote, so ahead and behind are against the last fetch.

use crate::git::git;
use herdr_github::pull_request::{Input, local_checkout};
use std::time::Instant;

/// Files listed per checkout; the rest are only counted.
pub const MAX_FILES: usize = 50;
/// Commits listed per checkout.
pub const MAX_COMMITS: usize = 5;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Checkout {
    /// The checkout's top-level directory.
    pub path: String,
    /// The Git common directory its worktrees share, which names the
    /// repository whatever worktree this is.
    pub common_dir: String,
    /// `None` on a detached HEAD.
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// The first [`MAX_FILES`] changed files, in Git's order.
    pub files: Vec<FileChange>,
    /// Every changed file, listed or not.
    pub total_files: usize,
    /// Newest first, at most [`MAX_COMMITS`].
    pub commits: Vec<Commit>,
}

impl Checkout {
    pub fn dirty(&self) -> bool {
        self.total_files > 0
    }

    /// Lines added and removed across the listed files.
    pub fn lines(&self) -> (u64, u64) {
        self.files.iter().fold((0, 0), |(added, removed), file| {
            (
                added + file.additions.unwrap_or_default(),
                removed + file.deletions.unwrap_or_default(),
            )
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Added,
    Conflicted,
    Deleted,
    Modified,
    Renamed,
    Untracked,
}

impl Kind {
    /// The one-letter mark `git status --short` uses.
    pub fn mark(self) -> &'static str {
        match self {
            Self::Added => "A",
            Self::Conflicted => "U",
            Self::Deleted => "D",
            Self::Modified => "M",
            Self::Renamed => "R",
            Self::Untracked => "?",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub kind: Kind,
    /// `None` for untracked and binary files, which have no lines to count.
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub sha: String,
    pub subject: String,
    pub author: String,
    /// Committer time, in seconds since the Unix epoch.
    pub time: i64,
}

/// Reads `input`'s checkout, resolved and verified against its branch as
/// every other read of a workspace's checkout is.
pub fn read(
    input: &Input,
    deadline: Instant,
    cancelled: &impl Fn() -> bool,
) -> crate::Result<Checkout> {
    let path = local_checkout(input, deadline, cancelled)?;
    let common_dir = git(
        &path,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        "find the repository",
        deadline,
        cancelled,
    )?;
    let status = git(
        &path,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "-z",
            "--untracked-files=normal",
        ],
        "read the working tree",
        deadline,
        cancelled,
    )?;
    let Parsed {
        branch,
        upstream,
        ahead,
        behind,
        mut files,
        born,
    } = parse_status(&status);
    let mut commits = Vec::new();
    // An unborn branch has no HEAD to count lines against or log from.
    if born {
        // Counts are a nicety: a failure keeps the files without them.
        if let Ok(numstat) = git(
            &path,
            &["diff", "--numstat", "-z", "HEAD"],
            "count changed lines",
            deadline,
            cancelled,
        ) {
            apply_numstat(&mut files, &numstat);
        }
        let log = git(
            &path,
            &[
                "log",
                &format!("--max-count={MAX_COMMITS}"),
                "-z",
                "--format=%h%x1f%ct%x1f%an%x1f%s",
            ],
            "read recent commits",
            deadline,
            cancelled,
        )?;
        commits = parse_log(&log);
    }
    let total_files = files.len();
    files.truncate(MAX_FILES);
    Ok(Checkout {
        path,
        common_dir,
        branch,
        upstream,
        ahead,
        behind,
        files,
        total_files,
        commits,
    })
}

/// `git status --porcelain=v2 --branch -z`, before line counts and commits.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Parsed {
    pub(crate) branch: Option<String>,
    pub(crate) upstream: Option<String>,
    pub(crate) ahead: u32,
    pub(crate) behind: u32,
    pub(crate) files: Vec<FileChange>,
    /// Whether HEAD names a commit.
    pub(crate) born: bool,
}

pub(crate) fn parse_status(text: &str) -> Parsed {
    let mut parsed = Parsed {
        born: true,
        ..Parsed::default()
    };
    let mut records = text.split('\0');
    while let Some(record) = records.next() {
        if let Some(header) = record.strip_prefix("# ") {
            parse_header(&mut parsed, header);
            continue;
        }
        let (kind, path) = match record.split_once(' ') {
            Some(("1", rest)) => (ordinary(rest), path_after(rest, 7)),
            // A rename's original path follows in the next record.
            Some(("2", rest)) => {
                let _ = records.next();
                (Kind::Renamed, path_after(rest, 8))
            }
            Some(("u", rest)) => (Kind::Conflicted, path_after(rest, 9)),
            Some(("?", path)) => (Kind::Untracked, Some(path)),
            _ => continue,
        };
        let Some(path) = path else {
            continue;
        };
        parsed.files.push(FileChange {
            path: path.to_owned(),
            kind,
            additions: None,
            deletions: None,
        });
    }
    parsed
}

fn parse_header(parsed: &mut Parsed, header: &str) {
    match header.split_once(' ') {
        Some(("branch.oid", oid)) => parsed.born = oid != "(initial)",
        Some(("branch.head", head)) => {
            parsed.branch = (head != "(detached)").then(|| head.to_owned());
        }
        Some(("branch.upstream", upstream)) => parsed.upstream = Some(upstream.to_owned()),
        Some(("branch.ab", counts)) => {
            for count in counts.split(' ') {
                if let Some(ahead) = count.strip_prefix('+') {
                    parsed.ahead = ahead.parse().unwrap_or_default();
                } else if let Some(behind) = count.strip_prefix('-') {
                    parsed.behind = behind.parse().unwrap_or_default();
                }
            }
        }
        _ => {}
    }
}

/// An entry's path: what follows its first `fields` space-separated fields.
fn path_after(rest: &str, fields: usize) -> Option<&str> {
    rest.splitn(fields + 1, ' ').nth(fields)
}

/// An ordinary entry's kind from its staged and unstaged letters, the
/// first field.
fn ordinary(rest: &str) -> Kind {
    let xy = rest.split(' ').next().unwrap_or_default();
    if xy.contains('A') {
        Kind::Added
    } else if xy.contains('D') {
        Kind::Deleted
    } else {
        Kind::Modified
    }
}

/// `git diff --numstat -z HEAD`'s counts, set on the files they name. A
/// rename's record leaves its path empty and gives both paths after it.
pub(crate) fn apply_numstat(files: &mut [FileChange], text: &str) {
    let mut records = text.split('\0');
    while let Some(record) = records.next() {
        let mut fields = record.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let path = if path.is_empty() {
            let _ = records.next();
            records.next().unwrap_or_default()
        } else {
            path
        };
        if let Some(file) = files.iter_mut().find(|file| file.path == path) {
            // Binary files print `-` for both.
            file.additions = added.parse().ok();
            file.deletions = removed.parse().ok();
        }
    }
}

pub(crate) fn parse_log(text: &str) -> Vec<Commit> {
    text.split('\0')
        .filter_map(|record| {
            let mut fields = record.trim_start_matches('\n').splitn(4, '\u{1f}');
            Some(Commit {
                sha: fields.next().filter(|sha| !sha.is_empty())?.to_owned(),
                time: fields.next()?.parse().ok()?,
                author: fields.next()?.to_owned(),
                subject: fields.next()?.to_owned(),
            })
        })
        .take(MAX_COMMITS)
        .collect()
}

#[cfg(test)]
mod tests;
