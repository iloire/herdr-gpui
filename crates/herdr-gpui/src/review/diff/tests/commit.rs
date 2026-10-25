//! Reviewing one commit on its own, against a real repository: what it
//! changed and nothing else, read whichever branch is checked out now.
use super::*;
use crate::{
    browser::Location,
    review::{
        Error as ReviewError,
        diff::commit::{empty_tree, parse_shown},
    },
};
use std::{path::Path, process::Command};

struct Repo {
    directory: tempfile::TempDir,
    _hooks: tempfile::TempDir,
    /// The root commit, then the one after it.
    root: CommitId,
    second: CommitId,
}

fn git(directory: &Path, hooks: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(["-c", "user.name=Ann Author"])
        .args(["-c", "user.email=test@example.invalid"])
        .args(["-c", "commit.gpgsign=false"])
        .arg("-c")
        .arg(format!("core.hooksPath={}", hooks.display()))
        .args(args)
        .env_remove("GIT_DIR")
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// Thirty numbered lines, `changed` replaced.
fn numbered(changed: &[(usize, &str)]) -> String {
    (1..=30)
        .map(|n| {
            changed
                .iter()
                .find(|(line, _)| *line == n)
                .map_or(format!("line {n}\n"), |(_, text)| format!("{text}\n"))
        })
        .collect()
}

/// A repository with two commits on `feature`, then switched to `other`
/// with the working tree edited and an untracked file, so neither the
/// branch nor the files on disk are the commit's.
fn repository() -> Repo {
    let directory = tempfile::tempdir().unwrap();
    let hooks = tempfile::tempdir().unwrap();
    let path = directory.path();
    let git = |args: &[&str]| git(path, hooks.path(), args);
    let write = |name: &str, text: &str| std::fs::write(path.join(name), text).unwrap();
    git(&["init", "-q", "-b", "feature"]);
    write("lib.rs", &numbered(&[]));
    write("gone.rs", "bye\n");
    git(&["add", "-A"]);
    git(&["commit", "-qm", "root"]);
    let root = git(&["rev-parse", "HEAD"]);
    write("lib.rs", &numbered(&[(25, "line 25 changed")]));
    std::fs::remove_file(path.join("gone.rs")).unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "fix: change line 25"]);
    let second = git(&["rev-parse", "HEAD"]);
    git(&["switch", "-qc", "other"]);
    write("lib.rs", &numbered(&[(2, "working tree"), (25, "later")]));
    write("untracked.rs", "fresh\n");
    Repo {
        directory,
        _hooks: hooks,
        root: CommitId::try_from(root).unwrap(),
        second: CommitId::try_from(second).unwrap(),
    }
}

/// The review names the branch it was opened on, which moved on since.
fn input(repo: &Repo) -> Input {
    let path = repo.directory.path().canonicalize().unwrap();
    Input {
        checkout: Some(path.to_str().unwrap().to_owned()),
        repo_key: Some(path.join(".git").to_str().unwrap().to_owned()),
        branch: "feature".into(),
    }
}

fn read(repo: &Repo, commit: &CommitId) -> Loaded {
    let mut loaded = load(
        &input(repo),
        Scope::Commit(commit.clone()),
        None,
        None,
        false,
    )
    .unwrap();
    loaded.read_all();
    loaded
}

fn texts(loaded: &Loaded, kind: Kind) -> Vec<&str> {
    loaded
        .diff
        .files
        .iter()
        .filter_map(FileDiff::lines)
        .flat_map(|lines| {
            lines
                .iter()
                .filter(move |line| line.kind == kind)
                .map(|line| lines.text_of(line))
        })
        .collect()
}

#[test]
fn a_commit_shows_its_own_changes_whatever_is_checked_out() {
    let repo = repository();
    let loaded = read(&repo, &repo.second);
    let files: Vec<_> = loaded
        .diff
        .files
        .iter()
        .map(|file| (file.path.as_str(), file.status))
        .collect();
    // No untracked file and no working tree edit: those are not the commit's.
    assert_eq!(
        files,
        [("gone.rs", Status::Deleted), ("lib.rs", Status::Modified)]
    );
    assert_eq!(texts(&loaded, Kind::Added), ["line 25 changed"]);
    assert_eq!(texts(&loaded, Kind::Removed), ["bye", "line 25"]);
    assert_eq!(loaded.source.target(), Some(repo.second.as_str()));
    assert_eq!(loaded.diff.before, repo.root.short());
    assert_eq!(
        loaded.summary,
        Some(Summary {
            subject: "fix: change line 25".into(),
            author: "Ann Author".into(),
        })
    );
}

#[test]
fn a_root_commit_adds_every_file_it_holds() {
    let repo = repository();
    let loaded = read(&repo, &repo.root);
    assert!(
        loaded
            .diff
            .files
            .iter()
            .all(|file| file.status == Status::Added)
    );
    assert_eq!(loaded.diff.files.len(), 2);
    assert_eq!(loaded.diff.before, "4b825dc");
}

#[test]
fn context_above_a_hunk_comes_from_the_commit_not_the_working_tree() {
    let repo = repository();
    let checkout = input(&repo).checkout.unwrap();
    let lines = read_lines(&checkout, Some(repo.second.as_str()), "lib.rs", 1..4).unwrap();
    assert_eq!(lines, ["line 1", "line 2", "line 3"]);
    let working = read_lines(&checkout, None, "lib.rs", 1..4).unwrap();
    assert_eq!(working, ["line 1", "working tree", "line 3"]);
    // A file the commit does not hold has nothing to show.
    assert_eq!(
        read_lines(&checkout, Some(repo.second.as_str()), "gone.rs", 1..2),
        None
    );
}

#[test]
fn a_checkout_of_another_repository_is_refused() {
    let repo = repository();
    let elsewhere = repository();
    let mut wrong = input(&repo);
    wrong.repo_key = input(&elsewhere).repo_key;
    let refused = load(
        &wrong,
        Scope::Commit(repo.second.clone()),
        None,
        None,
        false,
    );
    assert!(matches!(
        refused,
        Err(crate::Error::GitHub(herdr_github::Error::PullRequest(
            crate::pull_request::Error::RepositoryMismatch
        )))
    ));
}

#[test]
fn a_merge_is_compared_with_its_first_parent() {
    let first = "a".repeat(40);
    let second = "b".repeat(40);
    let (parent, summary) = parse_shown(&format!(
        "{first} {second}\u{1f}Ann\u{1b}[31m\u{1f}Merge branch 'x'"
    ));
    assert_eq!(parent.as_deref(), Some(first.as_str()));
    // Commit text is untrusted: no escapes reach the header.
    assert_eq!(summary.author, "Ann[31m");
    assert_eq!(summary.subject, "Merge branch 'x'");
    assert_eq!(parse_shown("\u{1f}Ann\u{1f}root").0, None);
    assert_eq!(empty_tree("sha256\n").len(), 64);
}

#[test]
fn only_a_full_lowercase_object_name_is_a_commit() {
    for name in ["a".repeat(40), "0123456789abcdef".repeat(4), "f".repeat(64)] {
        assert!(CommitId::try_from(name).is_ok());
    }
    for name in [
        String::new(),
        "abc1234".into(),
        "A".repeat(40),
        "g".repeat(40),
        "a".repeat(41),
        format!("-{}", "a".repeat(39)),
        "HEAD".into(),
        format!("{}..", "a".repeat(38)),
    ] {
        assert!(matches!(
            CommitId::try_from(name.clone()),
            Err(ReviewError::InvalidCommit)
        ));
    }
}

#[test]
fn a_commit_s_tab_is_saved_with_it_and_older_tabs_still_read() {
    let commit = "c".repeat(40);
    let checkout =
        serde_json::json!({ "repo_key": null, "branch": "main", "checkout": "/work/repo" });
    let saved = serde_json::json!({ "kind": "review", "checkout": checkout, "commit": commit });
    let location: Location = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(serde_json::to_value(&location).unwrap(), saved);
    assert_eq!(location.default_title(), "Commit \u{00b7} ccccccc");
    let Location::Review {
        commit: Some(read), ..
    } = &location
    else {
        panic!("not a commit's review");
    };
    assert_eq!(read.as_str(), commit);
    // A review saved before commits could be shown has none, and saves none.
    let older = serde_json::json!({ "kind": "review", "checkout": checkout });
    let location: Location = serde_json::from_value(older.clone()).unwrap();
    assert!(matches!(location, Location::Review { commit: None, .. }));
    assert_eq!(serde_json::to_value(&location).unwrap(), older);
    // A saved name that is not a commit's is refused, never passed to Git.
    let forged =
        serde_json::json!({ "kind": "review", "checkout": checkout, "commit": "--output=x" });
    assert!(serde_json::from_value::<Location>(forged).is_err());
}

#[test]
fn notes_on_a_commit_say_which_commit_their_lines_count_in() {
    let note = crate::review::notes::Note::new(
        Anchor::Line {
            path: "lib.rs".into(),
            side: Side::Added,
            number: 25,
            code: "line 25 changed".into(),
            before: None,
        },
        "Why?",
    )
    .unwrap();
    let text = crate::review::notes::prompt("/work/repo", Some("abc1234"), &[note]);
    assert!(text.starts_with("Review notes on commit abc1234 in /work/repo, from Herdr UI.\n"));
    assert!(text.contains("line numbers are commit abc1234's,"));
}
