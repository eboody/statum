//! Core traits and helper types shared by Statum crates.
//!
//! Most users reach these through the top-level `statum` crate. This crate
//! holds the small runtime surface that macro-generated code targets:
//!
//! - state marker traits
//! - transition capability traits
//! - runtime error and result types
//! - projection helpers for event-log style rebuilds

mod introspection;

pub mod projection;

pub use introspection::{
    MachineDescriptor, MachineGraph, MachineIntrospection, MachinePresentation,
    MachinePresentationDescriptor, MachineStateIdentity, MachineTransitionRecorder,
    RecordedTransition, StateDescriptor, StatePresentation, TransitionDescriptor,
    TransitionPresentation,
};

/// A generated state marker type.
///
/// Every `#[state]` variant produces one marker type that implements
/// `StateMarker`. The associated `Data` type is `()` for unit states and the
/// tuple payload type for data-bearing states.
pub trait StateMarker {
    /// The payload type stored in machines for this state.
    type Data;
}

/// A generated state marker with no payload.
///
/// Implemented for unit state variants like `Draft` or `Published`.
pub trait UnitState: StateMarker<Data = ()> {}

/// A generated state marker that carries payload data.
///
/// Implemented for tuple variants like `InReview(Assignment)`.
pub trait DataState: StateMarker {}

/// A machine that can transition directly to `Next`.
///
/// This is the stable trait-level view of `self.transition()`.
pub trait CanTransitionTo<Next> {
    /// The transition result type.
    type Output;

    /// Perform the transition.
    fn transition_to(self) -> Self::Output;
}

/// A machine that can transition using `Data`.
///
/// This is the stable trait-level view of `self.transition_with(data)`.
pub trait CanTransitionWith<Data> {
    /// The next state selected by this transition.
    type NextState;
    /// The transition result type.
    type Output;

    /// Perform the transition with payload data.
    fn transition_with_data(self, data: Data) -> Self::Output;
}

/// A machine that can transition by mapping its current state data into `Next`.
///
/// This is the stable trait-level view of `self.transition_map(...)`.
pub trait CanTransitionMap<Next: StateMarker> {
    /// The payload type stored in the current state.
    type CurrentData;
    /// The transition result type.
    type Output;

    /// Perform the transition by consuming the current state data and producing the next payload.
    fn transition_map<F>(self, f: F) -> Self::Output
    where
        F: FnOnce(Self::CurrentData) -> Next::Data;
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
