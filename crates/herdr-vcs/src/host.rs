//! Where a checkout's Git and files are read: this machine, or another host
//! through one long-lived `/bin/sh` of its own, over SSH or in a WSL
//! distribution, as listening ports scans one. Each Git child or file read
//! is one step through that shell, a round trip rather than an SSH
//! connection per Git command, and the host needs nothing but `sh` and
//! `git`. Readers take a [`Session`], so a feature reads every host through
//! one code path and cannot quietly be local-only.
//!
//! Blocking: a session belongs to one background worker, never the UI
//! thread. A step that overruns its deadline or limit breaks the host's
//! shell; the next read connects again, but not within [`RETRY`] of a
//! failed connection, so an unreachable host stays stale and never stalls
//! its worker.

mod error;
// The tests stand a local `/bin/sh` in for a host's.
#[cfg(all(test, unix))]
mod tests;

pub use error::Error;

use crate::{Result, code_index::MAX_SCANNED_BYTES};
use herdr_client::shell_quote;
use herdr_usage::{Host, Shell};
use std::{
    path::Path,
    time::{Duration, Instant},
};

/// How long a host whose shell could not start is left alone.
pub const RETRY: Duration = Duration::from_secs(30);
/// Output kept from one Git child, as the local runner keeps.
const OUTPUT: usize = 2 * 1024 * 1024;
/// Git's stderr kept as a failure's details; the rest is display noise.
const DETAILS: usize = 4096;
/// Separates a failed Git child's output from its stderr in a step's
/// answer. A file name can print it too, but a failure's output is never
/// parsed, so at most the shown details are wrong.
const STDERR: &str = "@@herdr-git-stderr";

/// One host's Git and files, for one worker.
pub struct Session {
    place: Place,
}

enum Place {
    Local,
    Remote {
        host: Host,
        /// Boxed: a running shell is large beside the local variant.
        shell: Option<Box<Shell>>,
        /// When the shell last failed to start.
        failed: Option<Instant>,
    },
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Session")
            .field("host", self.host())
            .finish_non_exhaustive()
    }
}

impl Session {
    /// A session on `host`. A remote host's shell starts at the first read.
    pub fn new(host: Host) -> Self {
        let place = match host {
            Host::Local => Place::Local,
            host => Place::Remote {
                host,
                shell: None,
                failed: None,
            },
        };
        Self { place }
    }

    pub fn local() -> Self {
        Self::new(Host::Local)
    }

    /// A session reading `host` through `shell`, which tests start on this
    /// machine to stand in for a remote one.
    #[cfg(any(test, feature = "test-support"))]
    pub fn through(host: Host, shell: Shell) -> Self {
        Self {
            place: Place::Remote {
                host,
                shell: Some(Box::new(shell)),
                failed: None,
            },
        }
    }

    pub fn host(&self) -> &Host {
        match &self.place {
            Place::Local => &Host::Local,
            Place::Remote { host, .. } => host,
        }
    }

    /// [`crate::git::git`] in `checkout` on this session's host.
    pub fn git(
        &mut self,
        checkout: &str,
        args: &[&str],
        operation: &'static str,
        deadline: Instant,
        cancelled: &impl Fn() -> bool,
    ) -> Result<String> {
        if matches!(self.place, Place::Local) {
            return crate::git::git(checkout, args, operation, deadline, cancelled);
        }
        let bytes = self.remote_git(checkout, args, operation, deadline, cancelled, OUTPUT)?;
        let text = String::from_utf8(bytes).map_err(|error| Error::Encoding {
            operation,
            source: error.utf8_error(),
        })?;
        Ok(text.trim_end_matches(['\r', '\n']).to_owned())
    }

    /// [`crate::git::git_bytes`] in `checkout` on this session's host.
    pub fn git_bytes(
        &mut self,
        checkout: &str,
        args: &[&str],
        operation: &'static str,
        deadline: Instant,
        cancelled: &impl Fn() -> bool,
        limit: usize,
    ) -> Result<Vec<u8>> {
        if matches!(self.place, Place::Local) {
            return crate::git::git_bytes(checkout, args, operation, deadline, cancelled, limit);
        }
        self.remote_git(checkout, args, operation, deadline, cancelled, limit)
    }

    /// A regular file's text, by the policy of
    /// [`crate::code_index::read_source`]: not through a symlink, at most
    /// 1 MiB, and UTF-8 without a NUL byte early on. `None` when the file is
    /// missing or is not such text.
    pub fn read_text(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Option<String>> {
        if matches!(self.place, Place::Local) {
            return Ok(crate::code_index::read_source(Path::new(path)));
        }
        // `head -c` reads one byte past the limit, so a file that grew
        // after it was measured is still refused rather than cut short.
        let step = format!(
            "herdr_f={path}
if [ -f \"$herdr_f\" ] && [ ! -h \"$herdr_f\" ] \
    && [ \"$(wc -c < \"$herdr_f\" | tr -d ' ')\" -le {MAX_SCANNED_BYTES} ]; then
    printf F; head -c {past} -- \"$herdr_f\"
else
    printf N
fi",
            path = shell_quote(path),
            past = MAX_SCANNED_BYTES + 1,
        );
        let limit = usize::try_from(MAX_SCANNED_BYTES + 2).unwrap_or(usize::MAX);
        let raw = self.step(&step, deadline, cancelled, limit)?;
        Ok(match raw.stdout.split_first() {
            Some((b'F', bytes)) => text(bytes),
            _ => None,
        })
    }

    /// Git in `checkout` through the host's shell, under the local policy:
    /// read-only, no file monitor, no prompts. Its stdout streams back as
    /// written; its stderr is held back and kept only as a failure's details.
    fn remote_git(
        &mut self,
        checkout: &str,
        args: &[&str],
        operation: &'static str,
        deadline: Instant,
        cancelled: &impl Fn() -> bool,
        limit: usize,
    ) -> Result<Vec<u8>> {
        let mut command = format!(
            "GIT_TERMINAL_PROMPT=0 NO_COLOR=1 git -c core.fsmonitor=false --no-optional-locks -C {}",
            shell_quote(checkout)
        );
        for arg in args {
            command.push(' ');
            command.push_str(&shell_quote(arg));
        }
        let step = format!(
            "{{ herdr_err=$( {{ {command} 1>&3; }} 2>&1 ); }} 3>&1
herdr_rc=$?
[ \"$herdr_rc\" -eq 0 ] || {{ printf '\\n{STDERR}\\n%s' \"$herdr_err\" | tail -c {DETAILS}; false; }}"
        );
        let raw = self.step(&step, deadline, cancelled, limit.saturating_add(DETAILS))?;
        if raw.success {
            if raw.stdout.len() > limit {
                return Err(Error::Size { operation }.into());
            }
            return Ok(raw.stdout);
        }
        let answer = String::from_utf8_lossy(&raw.stdout);
        let details = answer
            .rsplit_once(STDERR)
            .map_or("", |(_, details)| details)
            .trim();
        Err(crate::git::Error::Failed {
            operation,
            details: herdr_github::pull_request::clean(details),
        }
        .into())
    }

    /// Runs `step` in the host's shell within `deadline`, starting the shell
    /// first if it is not running. Any failure drops the shell, which may be
    /// left mid-answer.
    fn step(
        &mut self,
        step: &str,
        deadline: Instant,
        cancelled: &impl Fn() -> bool,
        limit: usize,
    ) -> Result<herdr_usage::probe::Raw> {
        if cancelled() {
            return Err(Error::Cancelled.into());
        }
        let Place::Remote {
            host,
            shell,
            failed,
        } = &mut self.place
        else {
            return Err(Error::Local.into());
        };
        if shell.is_none() {
            if failed.is_some_and(|at| at.elapsed() < RETRY) {
                return Err(Error::Retrying.into());
            }
            match connect(host) {
                Ok(started) => *shell = Some(Box::new(started)),
                Err(error) => {
                    *failed = Some(Instant::now());
                    return Err(error);
                }
            }
            *failed = None;
        }
        let Some(running) = shell.as_mut() else {
            return Err(Error::Retrying.into());
        };
        let timeout = deadline.saturating_duration_since(Instant::now());
        if timeout.is_zero() {
            return Err(Error::Shell(herdr_usage::Error::Timeout).into());
        }
        let answer = running.run_bytes(step, timeout, limit);
        if answer.is_err() {
            *shell = None;
        }
        answer.map_err(|error| Error::Shell(error).into())
    }
}

fn connect(host: &Host) -> Result<Shell> {
    let started = match host {
        Host::Local => return Err(Error::Local.into()),
        Host::Ssh(target) => Shell::connect(target),
        Host::Wsl(distro) => Shell::connect_wsl(distro),
        #[cfg(feature = "azure")]
        Host::Azure { .. } => return Err(crate::Error::CloudHostUnsupported),
    };
    started.map_err(|error| Error::Shell(error).into())
}

/// `bytes` as text, by the rules of `read_source`.
fn text(bytes: &[u8]) -> Option<String> {
    if bytes.len() as u64 > MAX_SCANNED_BYTES || bytes.iter().take(8000).any(|byte| *byte == 0) {
        return None;
    }
    String::from_utf8(bytes.to_vec()).ok()
}
