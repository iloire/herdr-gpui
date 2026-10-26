//! What a folder's Home shows of its repository beyond what the Changes
//! read holds: its most recent branches and how far each is from the
//! remote's default branch, when that branch last moved here, the latest
//! tag and the commits since it, and HEAD's commits per day and most
//! changed files over the last [`DAYS`] days. Read-only Git children
//! through the caller's [`Session`], so any host reads the same way;
//! nothing contacts a remote, so every count is against the last fetch.

use super::default_branch;
use crate::host::Session;
use std::{collections::HashMap, time::Instant};

/// Branches listed, most recently committed first.
pub const MAX_BRANCHES: usize = 8;
/// Days of activity counted, today included.
pub const DAYS: usize = 14;
/// Most changed files listed.
pub const MAX_HOT: usize = 5;
/// Commits read for activity, however busy the days were.
const MAX_LOGGED: usize = 1000;
const DAY: i64 = 86_400;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Overview {
    /// Local branches, most recently committed first.
    pub branches: Vec<Branch>,
    /// The remote's default branch the branches are counted against, as
    /// `origin/main`.
    pub base: Option<String>,
    /// When `base` last moved here, from its reflog: the last fetch that
    /// brought it new commits. Unix seconds.
    pub base_moved: Option<i64>,
    pub release: Option<Release>,
    /// HEAD's commits per day, oldest first; the last is the 24 hours up
    /// to the read. Always [`DAYS`] long.
    pub days: Vec<u32>,
    /// The files HEAD's commits of those days touched most, most first.
    pub hot: Vec<Hot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub subject: String,
    /// Its tip's committer time, in Unix seconds.
    pub time: i64,
    /// Whether HEAD is on it.
    pub current: bool,
    /// Commits it has that the base lacks, and the reverse; `None` without
    /// a base, or with a Git too old to count them in one read.
    pub ahead_behind: Option<(u32, u32)>,
}

impl Branch {
    /// Whether the base holds everything on it.
    pub fn merged(&self) -> bool {
        self.ahead_behind.is_some_and(|(ahead, _)| ahead == 0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// The newest tag HEAD descends from.
    pub tag: String,
    /// Its commit's committer time, in Unix seconds.
    pub time: i64,
    /// Commits on HEAD since it.
    pub since: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hot {
    pub path: String,
    pub commits: u32,
}

/// Reads the overview of the checkout at `path`, counting days back from
/// `now` (Unix seconds). Only listing the branches must succeed; each
/// other part is a nicety left empty when its read fails.
pub fn read(
    session: &mut Session,
    path: &str,
    now: i64,
    deadline: Instant,
    cancelled: &impl Fn() -> bool,
) -> crate::Result<Overview> {
    let base = default_branch(session, path, deadline, cancelled);
    let mut git =
        |args: &[&str], operation| session.git(path, args, operation, deadline, cancelled);
    let mut branches = |base: Option<&str>| {
        let args = branch_args(base);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        git(&args, "list branches")
    };
    // `ahead-behind` needs Git 2.41; an older one lists without counts.
    let branches = match base.as_deref() {
        Some(base) => branches(Some(base)).or_else(|_| branches(None))?,
        None => branches(None)?,
    };
    let base_moved = base.as_ref().and_then(|base| {
        let text = git(
            &["log", "-g", "-1", "--date=unix", "--format=%gd", base],
            "read when the default branch moved",
        )
        .ok()?;
        parse_reflog_time(&text)
    });
    let release = git(
        &["describe", "--tags", "--abbrev=0", "HEAD"],
        "find the latest tag",
    )
    .ok()
    .filter(|tag| !tag.is_empty())
    .and_then(|tag| {
        let reference = format!("refs/tags/{tag}");
        let time = git(
            &["log", "-1", "--format=%ct", &reference],
            "read the latest tag",
        )
        .ok()?
        .trim()
        .parse()
        .ok()?;
        let range = format!("{reference}..HEAD");
        let since = git(
            &["rev-list", "--count", &range],
            "count commits since the tag",
        )
        .ok()?
        .trim()
        .parse()
        .ok()?;
        Some(Release { tag, time, since })
    });
    let since = format!("--since={}", now - DAYS as i64 * DAY);
    let max = format!("--max-count={MAX_LOGGED}");
    let log = git(
        &[
            "-c",
            "core.quotePath=false",
            "log",
            &since,
            &max,
            "--name-only",
            "--format=%x1e%ct",
            "HEAD",
        ],
        "read recent activity",
    )
    .unwrap_or_default();
    let (days, hot) = parse_activity(&log, now);
    Ok(Overview {
        branches: parse_branches(&branches),
        base,
        base_moved,
        release,
        days,
        hot,
    })
}

/// `for-each-ref` over the local branches, with each one's distance from
/// `base` when given. A ref name cannot hold `)`, so it needs no escaping.
fn branch_args(base: Option<&str>) -> [String; 5] {
    let counts = base.map_or_else(String::new, |base| format!("%(ahead-behind:{base})"));
    [
        "for-each-ref".into(),
        "--sort=-committerdate".into(),
        format!("--count={MAX_BRANCHES}"),
        format!(
            "--format=%(HEAD)%1f%(refname:short)%1f%(committerdate:unix)%1f{counts}%1f%(contents:subject)"
        ),
        "refs/heads".into(),
    ]
}

/// `for-each-ref` records: `HEAD mark, name, time, ahead behind, subject`.
pub(crate) fn parse_branches(text: &str) -> Vec<Branch> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.splitn(5, '\u{1f}');
            let current = fields.next()? == "*";
            let name = fields.next().filter(|name| !name.is_empty())?.to_owned();
            let time = fields.next()?.parse().ok()?;
            let ahead_behind = fields
                .next()?
                .split_once(' ')
                .and_then(|(ahead, behind)| Some((ahead.parse().ok()?, behind.parse().ok()?)));
            Some(Branch {
                name,
                subject: fields.next()?.to_owned(),
                time,
                current,
                ahead_behind,
            })
        })
        .take(MAX_BRANCHES)
        .collect()
}

/// The time in a reflog selector printed with `--date=unix`, as
/// `origin/main@{1730000000}`.
pub(crate) fn parse_reflog_time(text: &str) -> Option<i64> {
    text.trim()
        .rsplit_once("@{")?
        .1
        .strip_suffix('}')?
        .parse()
        .ok()
}

/// `log --name-only --format=%x1e%ct`: each commit's time, then the files
/// it touched, counted into days back from `now` and per file.
pub(crate) fn parse_activity(text: &str, now: i64) -> (Vec<u32>, Vec<Hot>) {
    let mut days = vec![0; DAYS];
    let mut files: HashMap<&str, u32> = HashMap::new();
    for record in text.split('\u{1e}') {
        let mut lines = record.lines();
        let Some(time) = lines
            .next()
            .and_then(|time| time.trim().parse::<i64>().ok())
        else {
            continue;
        };
        let back = usize::try_from((now - time).max(0) / DAY).unwrap_or(usize::MAX);
        if let Some(day) = DAYS.checked_sub(back + 1) {
            days[day] += 1;
        }
        for path in lines.filter(|line| !line.is_empty()) {
            *files.entry(path).or_default() += 1;
        }
    }
    let mut hot: Vec<Hot> = files
        .into_iter()
        .map(|(path, commits)| Hot {
            path: path.to_owned(),
            commits,
        })
        .collect();
    hot.sort_by(|a, b| b.commits.cmp(&a.commits).then_with(|| a.path.cmp(&b.path)));
    hot.truncate(MAX_HOT);
    (days, hot)
}

#[cfg(test)]
mod tests;
