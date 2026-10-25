//! Reading a file to edit and writing it back. Both block, so they run on
//! the background executor. An agent may write the same file meanwhile, so
//! saving first reads the file again and refuses when it is no longer what
//! the editor started from; the user then picks between their text and the
//! file's. The write goes to a temporary file beside it, renamed over it,
//! so a crash never leaves half a file, and keeps the file's permissions.

use super::Error;
use std::{io::Write, path::Path};

/// Lines longer than this are generated or minified, and too costly to lay
/// out on every keystroke.
const MAX_LINE: usize = 10_000;

/// A file as the editor holds it: its text with `\n` endings, and whether
/// it was written with `\r\n`, to write it back the same way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Loaded {
    pub(crate) text: String,
    pub(crate) crlf: bool,
}

impl Loaded {
    fn of(raw: String) -> Result<Self, Error> {
        if raw.lines().any(|line| line.len() > MAX_LINE) {
            return Err(Error::LongLines);
        }
        let crlf = raw.contains("\r\n");
        let text = if crlf { raw.replace("\r\n", "\n") } else { raw };
        Ok(Self { text, crlf })
    }

    /// The bytes to write for `text`.
    fn encode(&self, text: &str) -> String {
        if self.crlf {
            text.replace('\n', "\r\n")
        } else {
            text.to_owned()
        }
    }
}

/// Reads `path` to edit. Blocking.
pub(crate) fn load(path: &Path) -> Result<Loaded, Error> {
    let raw = crate::code_index::read_source(path).ok_or(Error::Unreadable)?;
    Loaded::of(raw)
}

/// Writes `text` to `path`, unless the file is no longer `base`, the text
/// the editor started from; `force` writes over it all the same. Returns
/// what the file now holds, the next save's base. Blocking.
pub(crate) fn save(path: &Path, base: &Loaded, text: &str, force: bool) -> Result<Loaded, Error> {
    if !force {
        let now = load(path)?;
        if now != *base {
            return Err(Error::Changed);
        }
    }
    let saved = Loaded {
        text: text.to_owned(),
        crlf: base.crlf,
    };
    let folder = path.parent().ok_or(Error::Unreadable)?;
    let write = |source| Error::Write {
        path: path.to_owned(),
        source,
    };
    let mut temporary = tempfile::NamedTempFile::new_in(folder).map_err(write)?;
    temporary
        .write_all(saved.encode(text).as_bytes())
        .map_err(write)?;
    if let Ok(metadata) = std::fs::metadata(path) {
        temporary
            .as_file()
            .set_permissions(metadata.permissions())
            .map_err(write)?;
    }
    temporary
        .persist(path)
        .map_err(|error| write(error.error))?;
    Ok(saved)
}
