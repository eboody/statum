//! Core error and result types shared by Statum crates.

/// Errors returned by Statum runtime helpers.
#[derive(Debug)]
pub enum Error {
    /// Returned when a runtime check determines the current state is invalid.
    InvalidState,
}

/// Convenience result alias used by Statum APIs.
///
/// # Example
///
/// ```
/// fn ensure_ready(ready: bool) -> statum_core::Result<()> {
///     if ready {
///         Ok(())
///     } else {
///         Err(statum_core::Error::InvalidState)
///     }
/// }
///
/// assert!(ensure_ready(true).is_ok());
/// assert!(ensure_ready(false).is_err());
/// ```
pub type Result<T> = core::result::Result<T, Error>;

impl<T> From<Error> for core::result::Result<T, Error> {
    fn from(val: Error) -> Self {
        Err(val)
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl std::error::Error for Error {}
