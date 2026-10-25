use crate::listening_ports::{STEP_TIMEOUT, local_shell, parse, scan::COMMAND};
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};

/// A listener whose own environment lacks the workspace, as a server that
/// renames its process (`next-server`) looks on macOS, where `ps -E` reads
/// the overwritten argument area. The pane's variables are still in its
/// parent, so the listener is claimed through it.
#[test]
fn a_listener_without_the_variables_takes_its_parents() {
    if Command::new("python3").arg("-V").output().is_err() {
        return;
    }
    // The parent is Python rather than `sh`: macOS's `ps -E` cannot read the
    // environment of its own platform binaries. The listener inherits the
    // parent's stdin and stdout and exits when stdin closes.
    let mut parent = Command::new("python3")
        .arg("-c")
        .arg(
            "import os, subprocess, sys\n\
             env = {k: v for k, v in os.environ.items() if not k.startswith('HERDR_')}\n\
             subprocess.run([sys.executable, '-c', '''\n\
             import socket, sys\n\
             s = socket.socket()\n\
             s.bind((\"127.0.0.1\", 0))\n\
             s.listen()\n\
             print(s.getsockname()[1], flush=True)\n\
             sys.stdin.read()\n\
             '''], env=env)\n",
        )
        .env("HERDR_WORKSPACE_ID", "wParent")
        .env("HERDR_SOCKET_PATH", "/tmp/parent/herdr.sock")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(parent.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let number: u16 = line.trim().parse().unwrap();

    let output = local_shell().unwrap().run(COMMAND, STEP_TIMEOUT).unwrap();
    drop(parent.stdin.take());
    parent.wait().unwrap();

    if output.stdout.lines().any(|line| line == "N") {
        return;
    }
    let ports = parse(&output.stdout).unwrap();
    let owner = ports
        .iter()
        .find(|(_, listed)| listed.iter().any(|port| port.number == number))
        .map(|(owner, _)| owner);
    let owner = owner.unwrap_or_else(|| panic!("port {number} unclaimed in {:?}", output.stdout));
    assert_eq!(owner.workspace, "wParent");
    assert_eq!(owner.socket, "/tmp/parent/herdr.sock");
}
