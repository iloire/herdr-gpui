//! Failures reaching a host's checkouts through its shell.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("The read was cancelled.")]
    Cancelled,
    #[error("git {operation} printed text that is not UTF-8.")]
    Encoding {
        operation: &'static str,
        #[source]
        source: std::str::Utf8Error,
    },
    /// A step was asked of this machine, which runs Git directly.
    #[error("This machine has no host shell.")]
    Local,
    #[error("The host could not be reached a moment ago; trying again shortly.")]
    Retrying,
    #[error(transparent)]
    Shell(herdr_usage::Error),
    #[error("git {operation} printed more than can be read.")]
    Size { operation: &'static str },
    // keep-sorted end
}
