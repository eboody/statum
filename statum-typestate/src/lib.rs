//! Typestate-only Statum surface.
//!
//! This package intentionally keeps the public API small:
//!
//! - [`state`] for legal lifecycle phases
//! - [`machine`] for the typed machine and durable context
//! - [`transition`] for legal state changes
//! - the runtime traits and helper types those macros target
//!
//! Validators, machine introspection, machine references, and projection
//! helpers are not part of the documented end-user surface here.

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_doctests {}

#[doc(hidden)]
pub use statum_core::__private;
#[doc(inline)]
pub use statum_core::{
    Attested, Branch, CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error,
    Result, StateMarker, UnitState,
};
#[doc(hidden)]
pub use statum_core::{
    LinkedMachineGraph, LinkedReferenceTypeDescriptor, LinkedRelationBasis,
    LinkedRelationDescriptor, LinkedRelationKind, LinkedRelationSource, LinkedRelationTarget,
    LinkedStateDescriptor, LinkedTransitionDescriptor, LinkedTransitionInventory,
    LinkedValidatorEntryDescriptor, LinkedViaRouteDescriptor, MachineDescriptor, MachineGraph,
    MachineIntrospection, MachinePresentation, MachinePresentationDescriptor, MachineReference,
    MachineReferenceTarget, MachineRole, MachineStateIdentity, MachineTransitionRecorder,
    RebuildAttempt, RebuildReport, RecordedTransition, Rejection, StateDescriptor,
    StaticMachineLinkDescriptor, TransitionDescriptor, TransitionInventory, Validation,
};
#[doc(hidden)]
pub use statum_macros::__statum_emit_validator_methods_impl;

pub use statum_macros::machine;
pub use statum_macros::state;
pub use statum_macros::transition;
