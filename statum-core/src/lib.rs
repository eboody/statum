//! Core traits and helper types shared by Statum crates.
//!
//! Most users reach these through the top-level `statum` crate. This crate
//! holds the small runtime surface that macro-generated code targets:
//!
//! - state marker traits
//! - transition capability traits
//! - runtime error and result types
//! - projection helpers for event-log style rebuilds

use std::borrow::Cow;

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_doctests {}

mod introspection;

pub mod projection;

#[doc(hidden)]
pub mod __private {
    pub use crate::{
        MachinePresentation, MachinePresentationDescriptor, RebuildAttempt, RebuildReport,
        StatePresentation, TransitionPresentation, TransitionPresentationInventory,
    };
    pub use futures;
    pub use linkme;

    #[derive(Debug)]
    pub struct TransitionToken {
        _private: u8,
    }

    impl Default for TransitionToken {
        fn default() -> Self {
            Self::new()
        }
    }

    impl TransitionToken {
        pub const fn new() -> Self {
            Self { _private: 0 }
        }
    }
}

pub use introspection::{
    MachineDescriptor, MachineGraph, MachineIntrospection, MachinePresentation,
    MachinePresentationDescriptor, MachineStateIdentity, MachineTransitionRecorder,
    RecordedTransition, StateDescriptor, StatePresentation, TransitionDescriptor,
    TransitionInventory, TransitionPresentation, TransitionPresentationInventory,
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

/// A first-class two-way branching transition result.
///
/// This lets a transition expose two concrete machine targets while keeping the
/// branch alternatives visible to Statum introspection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Branch<A, B> {
    /// The first legal target branch.
    First(A),
    /// The second legal target branch.
    Second(B),
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

/// A structured validator rejection captured during typed rehydration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rejection {
    /// Stable machine-readable reason key for why the validator rejected.
    pub reason_key: &'static str,
    /// Optional human-readable message for debugging and reports.
    pub message: Option<Cow<'static, str>>,
}

impl Rejection {
    /// Create a rejection with a stable reason key and no message.
    pub const fn new(reason_key: &'static str) -> Self {
        Self {
            reason_key,
            message: None,
        }
    }

    /// Attach a human-readable message to this rejection.
    pub fn with_message(self, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            message: Some(message.into()),
            ..self
        }
    }
}

impl From<&'static str> for Rejection {
    fn from(reason_key: &'static str) -> Self {
        Self::new(reason_key)
    }
}

impl core::fmt::Display for Rejection {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.message {
            Some(message) => write!(fmt, "{}: {}", self.reason_key, message),
            None => write!(fmt, "{}", self.reason_key),
        }
    }
}

impl std::error::Error for Rejection {}

/// An opt-in validator result that carries structured rejection details.
pub type Validation<T> = core::result::Result<T, Rejection>;

/// One validator evaluation recorded during typed rehydration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RebuildAttempt {
    /// Rust method name of the validator that ran.
    pub validator: &'static str,
    /// Rust state-marker name the validator was checking.
    pub target_state: &'static str,
    /// Whether this validator matched and produced the rebuilt state.
    pub matched: bool,
    /// Stable machine-readable rejection key, when the validator exposed one.
    pub reason_key: Option<&'static str>,
    /// Optional human-readable rejection message, when the validator exposed one.
    pub message: Option<Cow<'static, str>>,
}

/// A typed rehydration result plus the validator attempts that produced it.
#[derive(Debug)]
pub struct RebuildReport<M> {
    /// Validator attempts in evaluation order.
    pub attempts: Vec<RebuildAttempt>,
    /// Final rebuild result.
    pub result: Result<M>,
}

impl<M> RebuildReport<M> {
    /// Returns the first matching validator attempt, if any.
    pub fn matched_attempt(&self) -> Option<&RebuildAttempt> {
        self.attempts.iter().find(|attempt| attempt.matched)
    }

    /// Consumes the report and returns the original rebuild result.
    pub fn into_result(self) -> Result<M> {
        self.result
    }
}

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
