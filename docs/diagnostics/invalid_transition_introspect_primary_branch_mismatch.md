# Transition Introspect Primary Branch Mismatch

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_transition_introspect_primary_branch_mismatch.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_transition_introspect_primary_branch_mismatch.stderr`

## Broken Example

```rust
#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition};

#[state]
enum State {
    Draft,
    Done,
}

#[machine]
struct Machine<State> {}

#[transition]
impl Machine<Draft> {
    #[introspect(return = Machine<Done>)]
    fn finish(self) -> ::core::result::Result<(), Machine<Done>> {
        Ok(())
    }
}

fn main() {}
```

## Compiler Output

```text
error: Error: transition method `Machine<Draft>::finish` returns an unsupported type.
       Found: `fn finish(self) -> ::core::result::Result<(), Machine<Done>>`
       Expected: `fn finish(self) -> ::core::result::Result<Machine<Done>, E>`
       Reason: even with `#[introspect(return = ...)]`, the written return type must still resolve to the impl target machine path or a supported wrapper around it
       Fix: move `Machine<Done>` into the primary branch, for example with `fn finish(self) -> ::core::result::Result<Machine<Done>, E>`, or return `Machine<Done>` directly if you do not need the wrapper.
       Primary branch: `()`
       Note: Supported strict introspection shapes are direct machine paths and supported `::core::option::Option<...>`, `::core::result::Result<..., E>`, and `::statum::Branch<..., ...>` wrappers around direct machine paths.
             Source-backed aliases may be expanded only to suggest an explicit `#[introspect(return = ...)]`; they are not accepted as authoritative transition contracts in strict mode. Imported aliases, macro-generated aliases, include-generated aliases, ambiguous aliases, and foreign machine paths are rejected.
       Help: add `#[introspect(return = Machine<NextState>)]` with a direct machine path and supported wrapper shape, or rewrite the signature to use that direct type.
             Source-backed alias expansion is diagnostics-only in strict mode.
  --> tests/ui/invalid_transition_introspect_primary_branch_mismatch.rs:25:24
   |
25 |     fn finish(self) -> ::core::result::Result<(), Machine<Done>> {
   |                        ^
```

## Corrected Example

```rust
use statum::{machine, state, transition};

#[state]
enum WorkflowState {
    Draft,
    Review,
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

#[transition]
impl WorkflowMachine<Draft> {
    fn submit(self) -> WorkflowMachine<Review> {
        self.transition_to()
    }
}
```

## Explanation

- Found: `fn finish(self) -> ::core::result::Result<(), Machine<Done>>`
- Expected: `fn finish(self) -> ::core::result::Result<Machine<Done>, E>`
- Fix: move `Machine<Done>` into the primary branch, for example with `fn finish(self) -> ::core::result::Result<Machine<Done>, E>`, or return `Machine<Done>` directly if you do not need the wrapper.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
