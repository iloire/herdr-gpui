#![allow(clippy::unwrap_used)]
use super::*;
use std::{path::Path, process::Command, time::Duration};

fn file(path: &str, kind: Kind) -> FileChange {
    FileChange {
        path: path.into(),
        kind,
        additions: None,
        deletions: None,
    }
}

#[test]
fn status_reads_the_branch_its_upstream_and_every_kind_of_entry() {
    let text = [
        "# branch.oid 0123456789abcdef0123456789abcdef01234567",
        "# branch.head feature",
        "# branch.upstream origin/feature",
        "# branch.ab +2 -5",
        "1 .M N... 100644 100644 100644 aaaa bbbb src/with space.rs",
        "1 A. N... 000000 100644 100644 0000 cccc added.rs",
        "1 .D N... 100644 100644 000000 dddd dddd gone.rs",
        "2 R. N... 100644 100644 100644 eeee eeee R100 new name.rs",
        "old name.rs",
        "u UU N... 100644 100644 100644 100644 ffff gggg hhhh both.rs",
        "? untracked.txt",
        "",
    ]
    .join("\0");
    let parsed = parse_status(&text);
    assert_eq!(parsed.branch.as_deref(), Some("feature"));
    assert_eq!(parsed.upstream.as_deref(), Some("origin/feature"));
    assert_eq!((parsed.ahead, parsed.behind), (2, 5));
    assert!(parsed.born);
    assert_eq!(
        parsed.files,
        vec![
            file("src/with space.rs", Kind::Modified),
            file("added.rs", Kind::Added),
            file("gone.rs", Kind::Deleted),
            file("new name.rs", Kind::Renamed),
            file("both.rs", Kind::Conflicted),
            file("untracked.txt", Kind::Untracked),
        ]
    );
}

#[test]
fn status_of_a_detached_or_unborn_head() {
    let detached = parse_status("# branch.oid abc\0# branch.head (detached)\0");
    assert_eq!(detached.branch, None);
    assert!(detached.born);
    let unborn = parse_status("# branch.oid (initial)\0# branch.head main\0? a\0");
    assert_eq!(unborn.branch.as_deref(), Some("main"));
    assert!(!unborn.born);
    assert_eq!(unborn.files, vec![file("a", Kind::Untracked)]);
}

#[test]
fn malformed_records_are_skipped() {
    let parsed = parse_status("1 .M short\0? ok\0garbage\0");
    assert_eq!(parsed.files, vec![file("ok", Kind::Untracked)]);
}

#[test]
fn numstat_counts_land_on_their_files_renames_and_binaries_included() {
    let mut files = vec![
        file("a.rs", Kind::Modified),
        file("new.rs", Kind::Renamed),
        file("image.png", Kind::Modified),
        file("untracked", Kind::Untracked),
    ];
    apply_numstat(
        &mut files,
        concat!(
            "3\t1\ta.rs\0",
            "4\t0\t\0old.rs\0new.rs\0",
            "-\t-\timage.png\0"
        ),
    );
    let counts: Vec<_> = files
        .iter()
        .map(|file| (file.additions, file.deletions))
        .collect();
    assert_eq!(
        counts,
        vec![
            (Some(3), Some(1)),
            (Some(4), Some(0)),
            (None, None),
            (None, None)
        ]
    );
}

#[test]
fn log_records_keep_subjects_with_separators_and_stop_at_the_limit() {
    let record = |n: usize| format!("{n:07x}\u{1f}1700000000\u{1f}Ann\u{1f}fix: a \u{1f} b");
    let text: Vec<_> = (0..MAX_COMMITS + 2).map(record).collect();
    let commits = parse_log(&text.join("\0"));
    assert_eq!(commits.len(), MAX_COMMITS);
    assert_eq!(
        commits[0],
        Commit {
            sha: "0000000".into(),
            subject: "fix: a \u{1f} b".into(),
            author: "Ann".into(),
            time: 1_700_000_000,
        }
    );
    assert!(parse_log("").is_empty());
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(15)
}

fn read_at(path: &Path) -> crate::Result<Checkout> {
    let located = locate(path.to_str().unwrap(), deadline(), &|| false)?;
    read(located, deadline(), &|| false)
}

struct Repo {
    directory: tempfile::TempDir,
    hooks: tempfile::TempDir,
}

impl Repo {
    fn new() -> Self {
        let repo = Self {
            directory: tempfile::tempdir().unwrap(),
            hooks: tempfile::tempdir().unwrap(),
        };
        repo.git(&["init", "-b", "main"]);
        repo
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }

    fn git(&self, args: &[&str]) {
        let result = Command::new("git")
            .arg("-C")
            .arg(self.path())
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
            ])
            .arg(format!("core.hooksPath={}", self.hooks.path().display()))
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }

    fn write(&self, name: &str, text: &str) {
        std::fs::write(self.path().join(name), text).unwrap();
    }

    fn read(&self) -> crate::Result<Checkout> {
        read_at(self.path())
    }
}

#[test]
fn a_real_checkout_reads_its_files_counts_upstream_and_commits() {
    let repo = Repo::new();
    repo.write("kept.txt", "one\ntwo\n");
    repo.write("moved.txt", "a\nb\nc\n");
    repo.write("removed.txt", "x\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "first"]);
    repo.git(&["remote", "add", "origin", "."]);
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    repo.git(&["branch", "--set-upstream-to=origin/main"]);
    repo.write("kept.txt", "one\n2\nthree\n");
    repo.git(&["commit", "-am", "second"]);
    repo.git(&["mv", "moved.txt", "renamed.txt"]);
    repo.git(&["rm", "-q", "removed.txt"]);
    repo.write("kept.txt", "one\n2\nthree\nfour\n");
    repo.write("new.txt", "new\n");

    let checkout = repo.read().unwrap();
    assert_eq!(checkout.branch.as_deref(), Some("main"));
    assert_eq!(checkout.upstream.as_deref(), Some("origin/main"));
    assert_eq!((checkout.ahead, checkout.behind), (1, 0));
    assert_eq!(
        Path::new(&checkout.path).canonicalize().unwrap(),
        repo.path().canonicalize().unwrap()
    );
    assert!(
        checkout.common_dir.ends_with(".git"),
        "{}",
        checkout.common_dir
    );
    let mut files: Vec<_> = checkout
        .files
        .iter()
        .map(|file| {
            (
                file.path.as_str(),
                file.kind,
                file.additions,
                file.deletions,
            )
        })
        .collect();
    files.sort_unstable_by_key(|file| file.0);
    assert_eq!(
        files,
        vec![
            ("kept.txt", Kind::Modified, Some(1), Some(0)),
            ("new.txt", Kind::Untracked, None, None),
            ("removed.txt", Kind::Deleted, Some(0), Some(1)),
            ("renamed.txt", Kind::Renamed, Some(0), Some(0)),
        ]
    );
    assert_eq!(checkout.total_files, 4);
    assert!(checkout.dirty());
    assert_eq!(checkout.lines(), (1, 1));
    let subjects: Vec<_> = checkout
        .commits
        .iter()
        .map(|c| c.subject.as_str())
        .collect();
    assert_eq!(subjects, ["second", "first"]);
}

#[test]
fn an_unborn_checkout_lists_its_files_without_commits() {
    let repo = Repo::new();
    repo.write("draft.txt", "draft\n");
    let checkout = repo.read().unwrap();
    assert_eq!(checkout.branch.as_deref(), Some("main"));
    assert!(checkout.commits.is_empty());
    assert_eq!(checkout.files, vec![file("draft.txt", Kind::Untracked)]);
}

#[test]
fn a_clean_checkout_without_upstream_is_not_dirty() {
    let repo = Repo::new();
    repo.write("a", "a\n");
    repo.git(&["add", "a"]);
    repo.git(&["commit", "-m", "only"]);
    let checkout = repo.read().unwrap();
    assert!(!checkout.dirty());
    assert_eq!(checkout.upstream, None);
    assert_eq!((checkout.ahead, checkout.behind), (0, 0));
    assert_eq!(checkout.commits.len(), 1);
}

#[test]
fn files_past_the_limit_are_counted_not_listed() {
    let repo = Repo::new();
    for index in 0..MAX_FILES + 3 {
        repo.write(&format!("file-{index:03}"), "x\n");
    }
    let checkout = repo.read().unwrap();
    assert_eq!(checkout.files.len(), MAX_FILES);
    assert_eq!(checkout.total_files, MAX_FILES + 3);
}

#[test]
fn worktree_records_skip_bare_repositories() {
    let text = [
        "worktree /srv/bare.git",
        "bare",
        "",
        "worktree /code/app",
        "HEAD 0123",
        "branch refs/heads/main",
        "",
        "worktree /code/app-feature",
        "HEAD 4567",
        "detached",
        "",
    ]
    .join("\0");
    assert_eq!(parse_worktrees(&text), ["/code/app", "/code/app-feature"]);
    assert!(parse_worktrees("").is_empty());
}

#[test]
fn a_subfolder_locates_its_checkout_and_every_worktree_is_found() {
    let repo = Repo::new();
    repo.write("a", "a\n");
    repo.git(&["add", "a"]);
    repo.git(&["commit", "-m", "first"]);
    std::fs::create_dir(repo.path().join("sub")).unwrap();
    let located = locate(
        repo.path().join("sub").to_str().unwrap(),
        deadline(),
        &|| false,
    )
    .unwrap();
    let canonical = |path: &str| Path::new(path).canonicalize().unwrap();
    assert_eq!(
        canonical(&located.path),
        repo.path().canonicalize().unwrap()
    );
    let others = tempfile::tempdir().unwrap();
    let linked = others.path().join("linked");
    repo.git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        "agent",
        linked.to_str().unwrap(),
    ]);
    let found: Vec<_> = worktrees(&located.path, deadline(), &|| false)
        .unwrap()
        .iter()
        .map(|path| canonical(path))
        .collect();
    assert_eq!(
        found,
        [
            repo.path().canonicalize().unwrap(),
            linked.canonicalize().unwrap()
        ]
    );
    // Both worktrees share one common directory, which names the repository.
    let from_linked = locate(linked.to_str().unwrap(), deadline(), &|| false).unwrap();
    assert_eq!(
        canonical(&from_linked.common_dir),
        canonical(&located.common_dir)
    );
    let outside = tempfile::tempdir().unwrap();
    assert!(locate(outside.path().to_str().unwrap(), deadline(), &|| false).is_err());
}

#[test]
fn a_folder_scan_finds_the_repositories_directly_inside_it() {
    let root = tempfile::tempdir().unwrap();
    for name in ["b-repo", "a-repo"] {
        std::fs::create_dir_all(root.path().join(name).join(".git")).unwrap();
    }
    std::fs::create_dir(root.path().join("plain")).unwrap();
    std::fs::create_dir_all(root.path().join("plain/nested/.git")).unwrap();
    // A linked worktree's `.git` is a file.
    std::fs::create_dir(root.path().join("linked")).unwrap();
    std::fs::write(root.path().join("linked/.git"), "gitdir: x\n").unwrap();
    let found: Vec<_> = repositories_in(root.path())
        .iter()
        .map(|path| {
            Path::new(path)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(found, ["a-repo", "b-repo", "linked"]);
    assert!(repositories_in(&root.path().join("missing")).is_empty());
}

#[test]
fn a_branch_without_upstream_counts_commits_missing_from_the_default_branch() {
    let repo = Repo::new();
    repo.write("a", "a\n");
    repo.git(&["add", "a"]);
    repo.git(&["commit", "-m", "first"]);
    repo.git(&["remote", "add", "origin", "."]);
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    repo.git(&["switch", "-q", "-c", "agent/task"]);
    for subject in ["one", "two"] {
        repo.write("a", subject);
        repo.git(&["commit", "-qam", subject]);
    }
    let checkout = repo.read().unwrap();
    assert_eq!(checkout.upstream, None);
    assert_eq!(
        checkout.unmerged,
        Some(Unmerged {
            base: "origin/main".into(),
            count: 2
        })
    );
    // On the default branch itself, tracking it, ahead says it all.
    repo.git(&["switch", "-q", "main"]);
    repo.git(&["branch", "--set-upstream-to=origin/main"]);
    assert_eq!(repo.read().unwrap().unmerged, None);
}
