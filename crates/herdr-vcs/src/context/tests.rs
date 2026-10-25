#![allow(clippy::unwrap_used)]
use super::*;
use std::{
    process::Command,
    time::{Duration, Instant},
};

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

    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new("git")
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
            .unwrap()
    }

    fn git(&self, args: &[&str]) {
        let result = self.run(args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.path().join(name), text).unwrap();
    }

    fn commit(&self, name: &str, text: &str, message: &str) {
        self.write(name, text);
        self.git(&["add", name]);
        self.git(&["commit", "-m", message]);
    }

    fn state(&self) -> State {
        let common = self.path().join(".git");
        state(self.path(), &common)
    }
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

#[test]
fn a_quiet_checkout_is_in_the_middle_of_nothing() {
    let repo = Repo::new();
    repo.commit("a.txt", "one\n", "first");
    assert_eq!(repo.state(), State::default());
}

#[test]
fn a_conflicted_rebase_reports_its_step() {
    let repo = Repo::new();
    repo.commit("a.txt", "base\n", "base");
    repo.git(&["checkout", "-b", "topic"]);
    repo.commit("a.txt", "topic\n", "topic one");
    repo.commit("b.txt", "more\n", "topic two");
    repo.git(&["checkout", "main"]);
    repo.commit("a.txt", "main\n", "main change");
    repo.git(&["checkout", "topic"]);
    // The first of the two commits conflicts, so the rebase stops there.
    let result = repo.run(&["rebase", "--merge", "main"]);
    assert!(!result.status.success());
    let stopped = repo.state().stopped;
    assert_eq!(stopped, Some(Stopped::Rebase { step: Some((1, 2)) }));
    assert_eq!(stopped.map(Stopped::label).as_deref(), Some("rebase 1/2"));
}

#[test]
fn a_conflicted_merge_is_stopped() {
    let repo = Repo::new();
    repo.commit("a.txt", "base\n", "base");
    repo.git(&["checkout", "-b", "topic"]);
    repo.commit("a.txt", "topic\n", "topic");
    repo.git(&["checkout", "main"]);
    repo.commit("a.txt", "main\n", "main");
    assert!(!repo.run(&["merge", "topic"]).status.success());
    assert_eq!(repo.state().stopped, Some(Stopped::Merge));
}

#[test]
fn stashes_are_counted() {
    let repo = Repo::new();
    repo.commit("a.txt", "one\n", "first");
    repo.write("a.txt", "two\n");
    repo.git(&["stash"]);
    repo.write("a.txt", "three\n");
    repo.git(&["stash"]);
    assert_eq!(repo.state().stashes, 2);
}

#[test]
fn git_crypt_is_locked_until_its_key_is_there() {
    let repo = Repo::new();
    repo.commit(
        ".gitattributes",
        "# secrets\n.env.local filter=git-crypt diff=git-crypt -text\n",
        "attributes",
    );
    assert!(repo.state().crypt_locked);
    let keys = repo.path().join(".git/git-crypt/keys");
    fs::create_dir_all(&keys).unwrap();
    fs::write(keys.join("default"), "key").unwrap();
    assert!(!repo.state().crypt_locked);
}

#[test]
fn a_commented_out_git_crypt_rule_is_not_locked() {
    let repo = Repo::new();
    repo.commit(
        ".gitattributes",
        "# .env filter=git-crypt\n*.rs text\n",
        "attributes",
    );
    assert!(!repo.state().crypt_locked);
}

#[test]
fn a_linked_worktree_reads_its_own_git_directory_and_its_siblings() {
    let repo = Repo::new();
    repo.commit("a.txt", "one\n", "first");
    let parent = tempfile::tempdir().unwrap();
    let linked = parent.path().join("linked");
    let linked_text = linked.to_str().unwrap();
    repo.git(&["worktree", "add", "-b", "side", linked_text]);
    fs::write(linked.join("a.txt"), "changed\n").unwrap();

    let context = read(linked_text, deadline(), &|| false).unwrap();
    assert_eq!(context.checkout.branch.as_deref(), Some("side"));
    assert!(context.checkout.dirty());
    assert_eq!(context.state, State::default());
    let siblings: Vec<_> = context
        .worktrees
        .iter()
        .map(|worktree| worktree.branch.as_deref())
        .collect();
    assert_eq!(siblings, [Some("main")]);

    // A merge stopped in the linked worktree is its own, not main's.
    fs::write(
        git_dir(&linked).unwrap().join("MERGE_HEAD"),
        "0000000000000000000000000000000000000000\n",
    )
    .unwrap();
    let common = repo.path().join(".git");
    assert_eq!(state(&linked, &common).stopped, Some(Stopped::Merge));
    assert_eq!(repo.state().stopped, None);
}

#[test]
fn a_directory_outside_git_is_an_error() {
    let folder = tempfile::tempdir().unwrap();
    let result = read(folder.path().to_str().unwrap(), deadline(), &|| false);
    assert!(matches!(result, Err(crate::Error::Git(_))));
}
