#![allow(clippy::unwrap_used)]
use super::*;
use crate::changes;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

/// A `/bin/sh` on this machine, read as a remote host's would be.
fn shell() -> Shell {
    let mut command = Command::new("/bin/sh");
    command
        .arg("-s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    Shell::start(command).unwrap()
}

fn remote() -> Session {
    Session::through(Host::Ssh("stand-in".into()), shell())
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(15)
}

const NEVER: fn() -> bool = || false;

/// A repository in a folder whose name needs quoting in a shell.
struct Repo {
    _parent: tempfile::TempDir,
    path: PathBuf,
}

impl Repo {
    fn new() -> Self {
        let parent = tempfile::tempdir().unwrap();
        let path = parent.path().join("it's a \"repo\" $HOME");
        std::fs::create_dir(&path).unwrap();
        let repo = Self {
            _parent: parent,
            path,
        };
        repo.git(&["init", "-q", "-b", "main"]);
        repo
    }

    fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn write(&self, name: &str, text: impl AsRef<[u8]>) {
        std::fs::write(self.path.join(name), text).unwrap();
    }

    fn dir(&self) -> &str {
        self.path.to_str().unwrap()
    }
}

fn read(session: &mut Session, directory: &str) -> changes::Checkout {
    let located = changes::locate(session, directory, deadline(), &NEVER).unwrap();
    changes::read(session, located, deadline(), &NEVER).unwrap()
}

#[test]
fn a_checkout_reads_the_same_through_a_host_shell_as_on_this_machine() {
    let repo = Repo::new();
    repo.write("kept.txt", "one\ntwo\n");
    repo.git(&["add", "kept.txt"]);
    repo.git(&["commit", "-q", "-m", "first: with 'quotes' and $dollars"]);
    repo.write("kept.txt", "one\nthree\nfour\n");
    repo.write("new file.txt", "x\n");
    let local = read(&mut Session::local(), repo.dir());
    let mut session = remote();
    let through = read(&mut session, repo.dir());
    assert_eq!(through, local);
    assert_eq!(through.total_files, 2);
    assert_eq!(
        through.commits[0].subject,
        "first: with 'quotes' and $dollars"
    );
    // The shell stays up for the next read.
    assert_eq!(read(&mut session, repo.dir()), local);
}

#[test]
fn git_output_comes_back_byte_for_byte_with_its_nuls() {
    let repo = Repo::new();
    repo.write("a", "a");
    repo.write("b c", "b");
    let listing = remote()
        .git_bytes(
            repo.dir(),
            &["ls-files", "-z", "--others"],
            "list files",
            deadline(),
            &NEVER,
            1024,
        )
        .unwrap();
    assert_eq!(listing, b"a\0b c\0");
}

#[test]
fn a_failing_git_child_reports_its_stderr_not_its_output() {
    let outside = tempfile::tempdir().unwrap();
    let error = remote()
        .git(
            outside.path().to_str().unwrap(),
            &["rev-parse", "--show-toplevel"],
            "find the checkout",
            deadline(),
            &NEVER,
        )
        .unwrap_err();
    let crate::Error::Git(crate::git::Error::Failed { operation, details }) = error else {
        panic!("{error:?}");
    };
    assert_eq!(operation, "find the checkout");
    assert!(details.contains("not a git repository"), "{details}");
}

#[test]
fn text_files_read_exactly_and_anything_else_reads_as_none() {
    let repo = Repo::new();
    repo.write("crlf.txt", "a\r\nb");
    repo.write("binary", b"a\0b");
    repo.write(
        "large",
        vec![b'a'; usize::try_from(MAX_SCANNED_BYTES).unwrap() + 1],
    );
    repo.write("latin1", b"caf\xe9");
    std::os::unix::fs::symlink(repo.path.join("crlf.txt"), repo.path.join("link")).unwrap();
    let mut session = remote();
    let mut read = |name: &str| {
        let path = repo.path.join(name);
        session
            .read_text(path.to_str().unwrap(), deadline(), &NEVER)
            .unwrap()
    };
    assert_eq!(read("crlf.txt").as_deref(), Some("a\r\nb"));
    for name in ["binary", "large", "latin1", "link", "missing", "."] {
        assert_eq!(read(name), None, "{name}");
    }
    // Each answer is the same as this machine's own reader gives.
    for name in ["crlf.txt", "binary", "large", "latin1", "link"] {
        let path = repo.path.join(name);
        assert_eq!(read(name), crate::code_index::read_source(&path), "{name}");
    }
}

#[test]
fn output_past_the_limit_is_refused_and_drops_the_shell() {
    let repo = Repo::new();
    repo.write("a", "a");
    let mut session = remote();
    // Just past the limit, the shell has read the whole answer and stays.
    assert!(matches!(
        session.git_bytes(
            repo.dir(),
            &["ls-files", "-z", "--others"],
            "list files",
            deadline(),
            &NEVER,
            1
        ),
        Err(crate::Error::Host(Error::Size { .. }))
    ));
    assert!(
        session
            .git(repo.dir(), &["status"], "read", deadline(), &NEVER)
            .is_ok()
    );
    // Far past it, the shell stops reading mid-answer and is dropped.
    for file in 0..200 {
        repo.write(&format!("{file:0>40}"), "");
    }
    let mut session = Session::through(Host::Wsl("no-such-distro".into()), shell());
    let list = |session: &mut Session| {
        session.git_bytes(
            repo.dir(),
            &["ls-files", "-z", "--others"],
            "list files",
            deadline(),
            &NEVER,
            0,
        )
    };
    assert!(matches!(
        list(&mut session),
        Err(crate::Error::Host(Error::Shell(herdr_usage::Error::Size)))
    ));
    // The broken shell is gone: the host's own shell cannot start here,
    // and once that fails the host is left alone for a while.
    assert!(matches!(
        list(&mut session),
        Err(crate::Error::Host(Error::Shell(_)))
    ));
    assert!(matches!(
        list(&mut session),
        Err(crate::Error::Host(Error::Retrying))
    ));
}

#[test]
fn a_cancelled_read_runs_nothing() {
    let repo = Repo::new();
    let error = remote()
        .git(
            repo.dir(),
            &["status"],
            "read the working tree",
            deadline(),
            &|| true,
        )
        .unwrap_err();
    assert!(matches!(error, crate::Error::Host(Error::Cancelled)));
}
