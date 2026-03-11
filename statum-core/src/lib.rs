//! Core error and result types shared by Statum crates.

/// A generated state marker type.
pub trait StateMarker {
    /// The payload type stored in machines for this state.
    type Data;
}

/// A generated state marker with no payload.
pub trait UnitState: StateMarker<Data = ()> {}

/// A generated state marker that carries payload data.
pub trait DataState: StateMarker {}

/// A machine that can transition directly to `Next`.
pub trait CanTransitionTo<Next> {
    /// The transition result type.
    type Output;

    /// Perform the transition.
    fn transition_to(self) -> Self::Output;
}

/// A machine that can transition using `Data`.
pub trait CanTransitionWith<Data> {
    /// The next state selected by this transition.
    type NextState;
    /// The transition result type.
    type Output;

    /// Perform the transition with payload data.
    fn transition_with_data(self, data: Data) -> Self::Output;
}

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
