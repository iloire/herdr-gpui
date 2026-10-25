//! The unchanged lines a hunk leaves out above it, read on request from the
//! working tree, or from the commit a review shows, so a hunk can be read in
//! more of its surroundings.
use super::{Kind, Lines, load::inside, parse::hunk_starts};
use std::{
    io::{BufRead, BufReader, Read as _},
    ops::Range,
    path::Path,
    time::{Duration, Instant},
};

/// How long reading a file out of a commit may take.
const SHOW_TIMEOUT: Duration = Duration::from_secs(10);

/// Lines shown per request, nearest the hunk first.
pub(crate) const EXPAND_LINES: u32 = 200;
/// A file read further than this for context is not read.
const MAX_CONTEXT_BYTES: u64 = 64 * 1024 * 1024;

/// The unchanged lines hidden above a hunk header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Gap {
    /// The header's line in its file.
    pub header: usize,
    /// Their numbers after the change.
    pub hidden: Range<u32>,
    /// What to add to a number after the change for the one before it.
    pub shift: i64,
}

impl Gap {
    pub(crate) fn len(&self) -> u32 {
        self.hidden.end.saturating_sub(self.hidden.start)
    }

    /// The lines one request reads: the ones nearest the hunk.
    pub(crate) fn next(&self) -> Range<u32> {
        self.hidden
            .end
            .saturating_sub(EXPAND_LINES)
            .max(self.hidden.start)..self.hidden.end
    }

    /// The number before the change of `number` after it.
    pub(crate) fn old(&self, number: u32) -> u32 {
        u32::try_from(i64::from(number) + self.shift).unwrap_or(0)
    }
}

impl Lines {
    /// The lines hidden above the hunk header at `header`, if any.
    pub(crate) fn gap(&self, header: usize) -> Option<Gap> {
        let line = self.get(header).filter(|line| line.kind == Kind::Hunk)?;
        let (old_start, new_start) = hunk_starts(self.text_of(line))?;
        // A new or deleted file has nothing on one side to show.
        if old_start == 0 || new_start == 0 {
            return None;
        }
        let shown = self
            .iter()
            .take(header)
            .filter_map(|line| line.new)
            .max()
            .unwrap_or(0);
        let hidden = shown.saturating_add(1)..new_start;
        (!hidden.is_empty()).then(|| Gap {
            header,
            hidden,
            shift: i64::from(old_start) - i64::from(new_start),
        })
    }
}

/// A hunk header `@@ -a,b +c,d @@ …` grown upwards by `added` lines.
pub(super) fn grown_header(header: &str, added: u32) -> Option<String> {
    let rest = header.strip_prefix("@@ ")?;
    let (ranges, tail) = rest.split_once(" @@").unwrap_or((rest, ""));
    let mut parts = ranges.split(' ');
    let range = |part: &str, sign: char| -> Option<(u32, u32)> {
        let part = part.strip_prefix(sign)?;
        let (start, length) = part.split_once(',').unwrap_or((part, "1"));
        Some((start.parse().ok()?, length.parse().ok()?))
    };
    let (old, old_length) = range(parts.next()?, '-')?;
    let (new, new_length) = range(parts.next()?, '+')?;
    Some(format!(
        "@@ -{},{} +{},{} @@{tail}",
        old.saturating_sub(added),
        old_length.saturating_add(added),
        new.saturating_sub(added),
        new_length.saturating_add(added),
    ))
}

/// Lines `range` of `path` in `checkout`, numbered from one: as it is in
/// `commit`, else in the working tree. Blocking. A link is never followed
/// out of the checkout.
pub(crate) fn read_lines(
    checkout: &str,
    commit: Option<&str>,
    path: &str,
    range: Range<u32>,
) -> Option<Vec<String>> {
    if !inside(path) {
        return None;
    }
    if let Some(commit) = commit {
        let blob = crate::git::git_bytes(
            checkout,
            &["cat-file", "blob", &format!("{commit}:{path}")],
            "read a file in the commit",
            Instant::now() + SHOW_TIMEOUT,
            &|| false,
            MAX_CONTEXT_BYTES as usize,
        )
        .ok()?;
        return numbered(blob.as_slice(), range);
    }
    let full = Path::new(checkout).join(path);
    let metadata = std::fs::symlink_metadata(&full).ok()?;
    if !metadata.is_file() {
        return None;
    }
    numbered(
        BufReader::new(std::fs::File::open(full).ok()?.take(MAX_CONTEXT_BYTES)),
        range,
    )
}

/// Lines `range` of what `reader` reads, numbered from one.
fn numbered(mut reader: impl BufRead, range: Range<u32>) -> Option<Vec<String>> {
    let mut lines = Vec::new();
    let mut buffer = Vec::new();
    let mut number = 0_u32;
    while number + 1 < range.end {
        buffer.clear();
        if reader.read_until(b'\n', &mut buffer).ok()? == 0 {
            break;
        }
        number += 1;
        if number >= range.start {
            let line = String::from_utf8_lossy(&buffer);
            lines.push(line.trim_end_matches(['\n', '\r']).to_owned());
        }
    }
    Some(lines)
}
