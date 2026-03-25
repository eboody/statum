//! Compile-time verified typestate workflows for Rust.
//!
//! Statum is for workflow and protocol models where representational
//! correctness matters. It helps keep invalid, undesirable, or not-yet-
//! validated states out of ordinary code.
//! In the same spirit as [`Option`] and [`Result`], it uses the type system to
//! make absence, failure, and workflow legality explicit instead of leaving
//! them in status fields and guard code. It generates typed state markers,
//! typed machines, transition helpers, and typed rehydration from stored data.
//!
//! # Mental Model
//!
//! - [`state`](macro@state) defines the legal phases.
//! - [`machine`](macro@machine) defines the durable context carried across phases.
//! - [`transition`](macro@transition) defines the legal edges between phases.
//! - [`validators`](macro@validators) rebuilds typed machines from persisted data.
//!
//! # Quick Start
//!
//! ```rust
//! use statum::{machine, state, transition};
//!
//! #[state]
//! enum CheckoutState {
//!     EmptyCart,
//!     ReadyToPay(OrderDraft),
//!     Paid,
//! }
//!
//! #[derive(Clone)]
//! struct OrderDraft {
//!     total_cents: u64,
//! }
//!
//! #[machine]
//! struct Checkout<CheckoutState> {
//!     id: String,
//! }
//!
//! #[transition]
//! impl Checkout<EmptyCart> {
//!     fn review(self, total_cents: u64) -> Checkout<ReadyToPay> {
//!         self.transition_with(OrderDraft { total_cents })
//!     }
//! }
//!
//! #[transition]
//! impl Checkout<ReadyToPay> {
//!     fn pay(self) -> Checkout<Paid> {
//!         self.transition()
//!     }
//! }
//!
//! fn main() {
//!     let cart = Checkout::<EmptyCart>::builder()
//!         .id("order-1".to_owned())
//!         .build();
//!
//!     let ready = cart.review(4200);
//!     assert_eq!(ready.state_data.total_cents, 4200);
//!
//!     let _paid = ready.pay();
//! }
//! ```

//!
//! # Typed Rehydration
//!
//! `#[validators]` lets you rebuild persisted rows back into typed machine
//! states:
//!
//! ```rust
//! use statum::{machine, state, validators, Error};
//!
//! #[state]
//! enum TaskState {
//!     Draft,
//!     InReview(String),
//!     Published,
//! }
//!
//! #[machine]
//! struct Task<TaskState> {
//!     id: u64,
//! }
//!
//! struct TaskRow {
//!     id: u64,
//!     status: &'static str,
//!     reviewer: Option<String>,
//! }
//!
//! #[validators(Task)]
//! impl TaskRow {
//!     fn is_draft(&self) -> statum::Result<()> {
//!         if self.status == "draft" {
//!             Ok(())
//!         } else {
//!             Err(Error::InvalidState)
//!         }
//!     }
//!
//!     fn is_in_review(&self) -> statum::Result<String> {
//!         if self.status == "in_review" {
//!             self.reviewer.clone().ok_or(Error::InvalidState)
//!         } else {
//!             Err(Error::InvalidState)
//!         }
//!     }
//!
//!     fn is_published(&self) -> statum::Result<()> {
//!         if self.status == "published" {
//!             Ok(())
//!         } else {
//!             Err(Error::InvalidState)
//!         }
//!     }
//! }
//!
//! fn main() -> statum::Result<()> {
//!     let row = TaskRow {
//!         id: 7,
//!         status: "in_review",
//!         reviewer: Some("alice".to_owned()),
//!     };
//!
//!     let row_id = row.id;
//!     let machine = row.into_machine().id(row_id).build()?;
//!     match machine {
//!         task::SomeState::InReview(task) => assert_eq!(task.state_data, "alice"),
//!         _ => panic!("expected in-review task"),
//!     }
//!     Ok(())
//! }
//! ```
//!
//! If you want explainable rebuild traces, validators can also return
//! [`Validation`]. Then `.build_report()` and `.build_reports()` populate [`RebuildAttempt::reason_key`] and
//! [`RebuildAttempt::message`] for failed matches while keeping the normal
//! `.into_result()` surface.
//!
//! # Compile-Time Gating
//!
//! Methods only exist on states where you define them.
//!
//! ```compile_fail
//! use statum::{machine, state};
//!
//! #[state]
//! enum LightState {
//!     Off,
//!     On,
//! }
//!
//! #[machine]
//! struct Light<LightState> {}
//!
//! let light = Light::<Off>::builder().build();
//! let _ = light.switch_off(); // no such method on Light<Off>
//! ```
//!
//! # Machine Introspection
//!
//! Statum can also expose the static machine structure as typed metadata.
//! This is useful when the same machine definition should drive:
//!
//! - CLI explainers
//! - generated docs
//! - graph exports
//! - exact transition assertions in tests
//! - runtime replay or debug tooling
//!
//! The important detail is that the graph is exact at the transition-site
//! level. A consumer can ask for the legal targets of one specific method on
//! one specific source state.
//!
//! The graph is derived from macro-expanded, cfg-pruned `#[transition]`
//! method signatures. Supported return shapes are direct machine returns plus
//! canonical wrapper paths around machine types:
//! `::core::option::Option<Machine<NextState>>`,
//! `::core::result::Result<Machine<NextState>, E>`, and
//! `::statum::Branch<Machine<Left>, Machine<Right>>`.
//! Unsupported custom decision enums, wrapper aliases, and differently-qualified
//! machine paths are rejected instead of approximated. Whole-item `#[cfg]`
//! gates are supported, but nested `#[cfg]` or `#[cfg_attr]` on `#[state]`
//! variants, variant payload fields, or `#[machine]` fields are rejected
//! because they would otherwise drift the generated metadata from the active
//! build.
//!
//! For small amounts of human-facing metadata, Statum can also generate a
//! `machine::PRESENTATION` constant from `#[present(...)]` attributes. Add
//! `#[presentation_types(...)]` on the machine when those attributes should
//! carry typed `metadata = ...` payloads instead of just labels and
//! descriptions.
//!
//! ```rust
//! use statum::{
//!     machine, state, transition, MachineIntrospection, MachineTransitionRecorder,
//! };
//!
//! #[state]
//! enum FlowState {
//!     Fetched,
//!     Accepted,
//!     Rejected,
//! }
//!
//! #[machine]
//! struct Flow<FlowState> {}
//!
//! #[transition]
//! impl Flow<Fetched> {
//!     fn validate(
//!         self,
//!         accept: bool,
//!     ) -> ::core::result::Result<Flow<Accepted>, Flow<Rejected>> {
//!         if accept {
//!             Ok(self.accept())
//!         } else {
//!             Err(self.reject())
//!         }
//!     }
//!
//!     fn accept(self) -> Flow<Accepted> {
//!         self.transition()
//!     }
//!
//!     fn reject(self) -> Flow<Rejected> {
//!         self.transition()
//!     }
//! }
//!
//! fn main() {
//!     let graph = <Flow<Fetched> as MachineIntrospection>::GRAPH;
//!     let validate = graph
//!         .transition_from_method(flow::StateId::Fetched, "validate")
//!         .unwrap();
//!
//!     assert_eq!(
//!         graph.legal_targets(validate.id).unwrap(),
//!         &[flow::StateId::Accepted, flow::StateId::Rejected]
//!     );
//!
//!     let event = <Flow<Fetched> as MachineTransitionRecorder>::try_record_transition_to::<
//!         Flow<Accepted>,
//!     >(Flow::<Fetched>::VALIDATE)
//!     .unwrap();
//!
//!     assert_eq!(event.chosen, flow::StateId::Accepted);
//! }
//! ```
//!
//! Transition ids are exact and typed, but they are exposed as generated
//! associated consts on the source-state machine type, such as
//! `Flow::<Fetched>::VALIDATE`.
//!
//! # Where To Look Next
//!
//! - Start with [`state`](macro@state), [`machine`](macro@machine), and
//!   [`transition`](macro@transition).
//! - For stored rows and database rebuilds, read [`validators`](macro@validators).
//! - For append-only event logs, use [`projection`] before validator rebuilds.
//! - The repository README and `docs/` directory contain longer guides and
//!   showcase applications.

#[cfg(doctest)]
#[doc = include_str!("../../README.md")]
mod root_readme_doctests {}

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod crate_readme_doctests {}

#[doc(hidden)]
pub use statum_core::__private;
#[doc(inline)]
pub use statum_core::projection;
#[doc(inline)]
pub use statum_core::{
    Branch, CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error,
    MachineDescriptor, MachineGraph, MachineIntrospection, MachinePresentation,
    MachinePresentationDescriptor, MachineStateIdentity, MachineTransitionRecorder, RebuildAttempt,
    RebuildReport, RecordedTransition, Rejection, Result, StateDescriptor, StateMarker,
    StatePresentation, TransitionDescriptor, TransitionInventory, TransitionPresentation,
    TransitionPresentationInventory, UnitState, Validation,
};

/// Define the legal lifecycle phases for a machine.
///
/// `#[state]` accepts enums with:
///
/// - unit variants like `Draft`
/// - single-field tuple variants like `InReview(Assignment)`
/// - named-field variants like `InReview { reviewer: String }`
///
/// It generates one marker type per variant plus the trait bounds Statum uses
/// for typed machines and transitions.
///
/// If you need derives, place them below `#[state]`.
///
/// ```rust
/// use statum::state;
///
/// #[state]
/// enum ReviewState {
///     Draft,
///     InReview(Reviewer),
///     Published,
/// }
///
/// #[derive(Clone)]
/// struct Reviewer {
///     name: String,
/// }
/// ```
pub use statum_macros::state;

/// Define a typed machine that carries durable context across states.
///
/// The machine must be a struct whose first generic parameter is the
/// `#[state]` enum family:
///
/// - `struct Task<TaskState> { ... }`
/// - `struct Payment<PaymentState> { ... }`
///
/// `#[machine]` generates:
///
/// - the typed `Machine<State>` surface
/// - a builder for new machines
/// - a machine-scoped `machine::SomeState` enum for matching rebuilt machines
/// - a compatibility alias `machine::State = machine::SomeState`
/// - a machine-scoped `machine::Fields` struct for heterogeneous batch rebuilds
///
/// If you need derives, place them below `#[machine]`.
///
/// ```rust
/// use statum::{machine, state};
///
/// #[state]
/// enum TaskState {
///     Draft,
///     Published,
/// }
///
/// #[machine]
/// struct Task<TaskState> {
///     id: u64,
/// }
///
/// fn main() {
///     let task = Task::<Draft>::builder().id(1).build();
///     assert_eq!(task.id, 1);
/// }
/// ```
pub use statum_macros::machine;

/// Validate and generate legal transitions for one source state.
///
/// Apply `#[transition]` to an `impl Machine<CurrentState>` block. Transition
/// methods consume `self` and return `Machine<NextState>` or exact wrapper
/// shapes around that same machine path such as
/// `::core::result::Result<Machine<NextState>, E>`,
/// `::core::option::Option<Machine<NextState>>`, or
/// `::statum::Branch<Machine<Left>, Machine<Right>>`.
///
/// Inside the impl, use:
///
/// - `self.transition()` for unit target states
/// - `self.transition_with(data)` for data-bearing target states
/// - `self.transition_map(|current| next_data)` when the next payload is built
///   from the current payload
///
/// ```rust
/// use statum::{machine, state, transition};
///
/// #[state]
/// enum LightState {
///     Off,
///     On,
/// }
///
/// #[machine]
/// struct Light<LightState> {}
///
/// #[transition]
/// impl Light<Off> {
///     fn switch_on(self) -> Light<On> {
///         self.transition()
///     }
/// }
///
/// fn main() {
///     let _light = Light::<Off>::builder().build().switch_on();
/// }
/// ```
pub use statum_macros::transition;

#[doc(hidden)]
pub use statum_macros::__statum_emit_validator_methods_impl;
/// Rebuild typed machines from persisted data.
///
/// `#[validators(Machine)]` is attached to an `impl PersistedRow` block. Statum
/// resolves the state family from the machine definition. Define one
/// `is_{state}` method per state variant:
///
/// - return `statum::Result<()>` or `statum::Validation<()>` for unit states
/// - return `statum::Result<StateData>` or `statum::Validation<StateData>` for
///   data-bearing states
///
/// The generated API includes:
///
/// - `row.into_machine()` for single-item rebuilds
/// - `.into_machines()` when all items share the same machine fields
/// - `.into_machines_by(|row| machine::Fields { ... })` when each item needs
///   different machine fields
/// - `.build_report()` / `.build_reports()` when you want rebuild attempts and
///   stable rejection details alongside the normal result
///
/// Machine fields are available by name inside validator bodies through
/// generated bindings. Persisted-row fields still live on `self`.
///
/// ```rust
/// use statum::{machine, state, validators, Error};
///
/// #[state]
/// enum TaskState {
///     Draft,
///     InReview(String),
///     Published,
/// }
///
/// #[machine]
/// struct Task<TaskState> {
///     id: u64,
/// }
///
/// struct TaskRow {
///     id: u64,
///     status: &'static str,
///     reviewer: Option<String>,
/// }
///
/// #[validators(Task)]
/// impl TaskRow {
///     fn is_draft(&self) -> statum::Result<()> {
///         if self.status == "draft" {
///             Ok(())
///         } else {
///             Err(Error::InvalidState)
///         }
///     }
///
///     fn is_in_review(&self) -> statum::Result<String> {
///         if self.status == "in_review" {
///             self.reviewer.clone().ok_or(Error::InvalidState)
///         } else {
///             Err(Error::InvalidState)
///         }
///     }
///
///     fn is_published(&self) -> statum::Result<()> {
///         if self.status == "published" {
///             Ok(())
///         } else {
///             Err(Error::InvalidState)
///         }
///     }
/// }
///
/// fn main() -> statum::Result<()> {
///     let row = TaskRow {
///         id: 7,
///         status: "draft",
///         reviewer: None,
///     };
///
///     let row_id = row.id;
///     let _task = row.into_machine().id(row_id).build()?;
///     Ok(())
/// }
/// ```
pub use statum_macros::validators;
