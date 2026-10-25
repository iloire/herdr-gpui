//! What the checkouts on this machine hold right now, for the Changes
//! section: finding the checkout a directory sits in, every worktree of a
//! repository, and the repositories under a folder; then, per checkout, its
//! branch and how far it is from its upstream, its commits not yet on the
//! remote's default branch, the files with uncommitted changes and their
//! line counts, and its latest commits. Read-only Git children run by the
//! caller off the UI thread; nothing here contacts a remote, so every count
//! is against the last fetch.

use crate::git::git;
use std::{path::Path, time::Instant};

/// Files listed per checkout; the rest are only counted.
pub const MAX_FILES: usize = 50;
/// Commits listed per checkout.
pub const MAX_COMMITS: usize = 5;
/// Repositories taken from one folder's scan.
pub const MAX_SCANNED: usize = 200;

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
    /// Commits on HEAD that the remote's default branch lacks, when that
    /// says more than `ahead` does: an agent's branch with no upstream, or
    /// one whose upstream is not the default branch.
    pub unmerged: Option<Unmerged>,
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

    /// Commits the default branch lacks, or 0.
    pub fn unmerged_count(&self) -> u32 {
        self.unmerged.as_ref().map_or(0, |unmerged| unmerged.count)
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unmerged {
    /// The remote's default branch, as `origin/main`.
    pub base: String,
    pub count: u32,
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

/// The checkout a directory sits in: its top level and the common
/// directory its repository's worktrees share.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Located {
    pub path: String,
    pub common_dir: String,
}

/// The checkout `directory` is in, from any folder inside it. A directory
/// outside Git, or in a bare repository, is an error.
pub fn locate(
    directory: &str,
    deadline: Instant,
    cancelled: &impl Fn() -> bool,
) -> crate::Result<Located> {
    let text = git(
        directory,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-common-dir",
        ],
        "find the checkout",
        deadline,
        cancelled,
    )?;
    let mut lines = text.lines();
    match (lines.next(), lines.next()) {
        (Some(path), Some(common_dir)) if !path.is_empty() => Ok(Located {
            path: path.to_owned(),
            common_dir: common_dir.to_owned(),
        }),
        _ => Err(crate::git::Error::Failed {
            operation: "find the checkout",
            details: "not in a working tree".into(),
        }
        .into()),
    }
}

/// Every worktree of the repository `checkout` belongs to, main checkout
/// included, whose folder is still there.
pub fn worktrees(
    checkout: &str,
    deadline: Instant,
    cancelled: &impl Fn() -> bool,
) -> crate::Result<Vec<String>> {
    let text = git(
        checkout,
        &["worktree", "list", "--porcelain", "-z"],
        "list worktrees",
        deadline,
        cancelled,
    )?;
    Ok(parse_worktrees(&text)
        .into_iter()
        .filter(|path| Path::new(path).is_dir())
        .collect())
}

/// The repositories directly inside `root`: each folder holding a `.git`
/// folder or file, sorted, at most [`MAX_SCANNED`]. Reads the folder only;
/// no Git runs.
pub fn repositories_in(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join(".git").exists())
        .filter_map(|path| path.to_str().map(str::to_owned))
        .collect();
    found.sort_unstable();
    found.truncate(MAX_SCANNED);
    found
}

/// Reads the checkout at `located`.
pub fn read(
    located: Located,
    deadline: Instant,
    cancelled: &impl Fn() -> bool,
) -> crate::Result<Checkout> {
    let Located { path, common_dir } = located;
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
    let mut unmerged = None;
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
        unmerged = default_branch(&path, deadline, cancelled)
            // The branch tracking the default branch already says this
            // through ahead.
            .filter(|base| upstream.as_ref() != Some(base))
            .and_then(|base| {
                let range = format!("{base}..HEAD");
                let count = git(
                    &path,
                    &["rev-list", "--count", &range],
                    "count unmerged commits",
                    deadline,
                    cancelled,
                )
                .ok()?
                .trim()
                .parse()
                .ok()?;
                Some(Unmerged { base, count })
            });
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
        unmerged,
        files,
        total_files,
        commits,
    })
}

/// The remote's default branch as last fetched: what `origin/HEAD` names,
/// else `origin/main` or `origin/master`, whichever exists.
fn default_branch(path: &str, deadline: Instant, cancelled: &impl Fn() -> bool) -> Option<String> {
    let named = git(
        path,
        &["rev-parse", "--abbrev-ref", "origin/HEAD"],
        "find the default branch",
        deadline,
        cancelled,
    )
    .ok()
    .filter(|name| !name.is_empty() && name != "origin/HEAD");
    named.or_else(|| {
        ["origin/main", "origin/master"]
            .into_iter()
            .find_map(|name| {
                let reference = format!("refs/remotes/{name}");
                git(
                    path,
                    &["rev-parse", "--verify", "--quiet", &reference],
                    "find the default branch",
                    deadline,
                    cancelled,
                )
                .ok()
                .map(|_| name.to_owned())
            })
    })
}

/// `git worktree list --porcelain -z`: each record's `worktree` path,
/// leaving out bare repositories, which have no working tree.
pub(crate) fn parse_worktrees(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut current: Option<&str> = None;
    let mut bare = false;
    // Records end with an empty field.
    for field in text.split('\0').chain(std::iter::once("")) {
        if field.is_empty() {
            if let Some(path) = current.take().filter(|_| !bare) {
                paths.push(path.to_owned());
            }
            bare = false;
        } else if let Some(path) = field.strip_prefix("worktree ") {
            current = Some(path);
        } else if field == "bare" {
            bare = true;
        }
    }
    paths
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
