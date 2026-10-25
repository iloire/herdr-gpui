use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("The file changed on disk since you started editing")]
    Changed,
    #[error("This file has lines too long to edit here; open it in the editor")]
    LongLines,
    #[error("This file is missing, larger than 1 MiB, or not UTF-8 text")]
    Unreadable,
    #[error("Could not save {}: {source}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    // keep-sorted end
}
