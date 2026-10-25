//! Sorting a checkout's changed files so the edits worth reading stand out:
//! code first, then tests, then docs, and last the files no one reads line
//! by line, such as lockfiles, build output, images, CAD models and printer
//! code, folded into one line until opened. `[changes] noise` adds patterns
//! to the last group.

use herdr_vcs::changes::FileChange;

/// What kind of file a change is to, in the order the section lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Group {
    Code,
    Tests,
    Docs,
    Noise,
}

impl Group {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Code => "Code",
            Self::Tests => "Tests",
            Self::Docs => "Docs",
            Self::Noise => "Generated & binary",
        }
    }
}

/// Extensions of files that are binary or written by a tool.
// keep-sorted start
const NOISE_EXTENSIONS: &[&str] = &[
    "3mf", "a", "avif", "bin", "bmp", "class", "dll", "dylib", "exe", "gcode", "gif", "gz", "ico",
    "jar", "jpeg", "jpg", "lock", "map", "mov", "mp3", "mp4", "o", "obj", "otf", "pdf", "png",
    "pyc", "so", "step", "stl", "stp", "svgz", "tar", "tgz", "ttf", "wasm", "wav", "webm", "webp",
    "woff", "woff2", "xz", "zip",
];
// keep-sorted end

/// Folders whose contents a build or a package manager writes.
// keep-sorted start
const NOISE_FOLDERS: &[&str] = &[
    ".next",
    "__pycache__",
    "build",
    "coverage",
    "dist",
    "node_modules",
    "target",
    "vendor",
];
// keep-sorted end

/// Extensions of prose.
// keep-sorted start
const DOC_EXTENSIONS: &[&str] = &["adoc", "markdown", "md", "mdx", "rst", "txt"];
// keep-sorted end

/// Which group `path` falls in; `extra` holds `[changes] noise` patterns.
pub(crate) fn group_of(path: &str, extra: &[String]) -> Group {
    let path = path.trim_end_matches('/');
    let name = path.rsplit('/').next().unwrap_or(path);
    let folders = || path.split('/').rev().skip(1);
    let extension = name
        .rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map(|(_, extension)| extension.to_ascii_lowercase());
    let matches = |pattern: &str| {
        if let Some(wanted) = pattern.strip_prefix("*.") {
            extension
                .as_deref()
                .is_some_and(|extension| extension.eq_ignore_ascii_case(wanted))
        } else if let Some(folder) = pattern.strip_suffix('/') {
            folders().any(|seen| seen == folder)
        } else {
            name == pattern
        }
    };
    if crate::review::lockfile(path)
        || name.ends_with(".min.js")
        || name.ends_with(".min.css")
        || extension
            .as_deref()
            .is_some_and(|extension| NOISE_EXTENSIONS.binary_search(&extension).is_ok())
        || folders().any(|folder| NOISE_FOLDERS.binary_search(&folder).is_ok())
        || extra.iter().any(|pattern| matches(pattern))
    {
        return Group::Noise;
    }
    let stem = name.split('.').next().unwrap_or(name);
    if folders().any(|folder| matches!(folder, "test" | "tests" | "spec" | "__tests__"))
        || stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with("_tests")
        || name.contains(".test.")
        || name.contains(".spec.")
        || stem == "tests"
    {
        return Group::Tests;
    }
    if folders().any(|folder| folder == "docs")
        || extension
            .as_deref()
            .is_some_and(|extension| DOC_EXTENSIONS.binary_search(&extension).is_ok())
        || matches!(stem, "LICENSE" | "CHANGELOG" | "README")
    {
        return Group::Docs;
    }
    Group::Code
}

/// `files` in their groups, in group order, each keeping Git's order.
pub(crate) fn grouped<'a>(
    files: &'a [FileChange],
    extra: &[String],
) -> Vec<(Group, Vec<&'a FileChange>)> {
    let mut groups: Vec<(Group, Vec<&FileChange>)> = Vec::new();
    for file in files {
        let group = group_of(&file.path, extra);
        match groups.iter_mut().find(|(seen, _)| *seen == group) {
            Some((_, list)) => list.push(file),
            None => groups.push((group, vec![file])),
        }
    }
    groups.sort_by_key(|(group, _)| *group);
    groups
}

/// A short tally of what a folded noise group holds: `stl ×7, gcode ×14`.
pub(crate) fn tally(files: &[&FileChange]) -> String {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for file in files {
        let path = file.path.trim_end_matches('/');
        let name = path.rsplit('/').next().unwrap_or(path);
        let kind = if file.path.ends_with('/') {
            "folder".to_owned()
        } else {
            name.rsplit_once('.')
                .filter(|(stem, _)| !stem.is_empty())
                .map_or_else(|| name.to_owned(), |(_, ext)| ext.to_ascii_lowercase())
        };
        match counts.iter_mut().find(|(seen, _)| *seen == kind) {
            Some((_, count)) => *count += 1,
            None => counts.push((kind, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    counts
        .iter()
        .take(4)
        .map(|(kind, count)| format!("{kind} ×{count}"))
        .collect::<Vec<_>>()
        .join(", ")
}
