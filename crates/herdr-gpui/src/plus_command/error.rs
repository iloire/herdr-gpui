//! Why "+" did not open the command's tab.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("plus_command must be one line of at most 1024 bytes")]
    Command,
    #[error("Herdr UI is not connected")]
    NotConnected,
    #[error("no workspace is focused")]
    NoWorkspace,
    #[error("{0}")]
    Request(#[source] std::sync::Arc<crate::Error>),
    #[error("Herdr's answer did not name the new tab")]
    Response,
    #[error("another tab is still opening")]
    SlotBusy,
    #[error("Herdr did not open the tab in time")]
    Timeout,
    // keep-sorted end
}
