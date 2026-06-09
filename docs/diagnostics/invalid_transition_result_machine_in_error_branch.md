# Transition Result Machine In Error Branch

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_transition_result_machine_in_error_branch.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_transition_result_machine_in_error_branch.stderr`

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
       Reason: expected the impl target machine path directly, a source-backed type alias that expands to it, or that same machine path wrapped in a supported `Option`, `Result`, or `Branch` shape
       Fix: move `Machine<Done>` into the primary branch, for example with `fn finish(self) -> ::core::result::Result<Machine<Done>, E>`, or return `Machine<Done>` directly if you do not need the wrapper.
       Primary branch: `()`
       Note: Supported wrappers are `::core::option::Option<...>`, `::core::result::Result<..., E>`, and `::statum::Branch<..., ...>`, plus ordinary source-declared type aliases that expand to those shapes.
             Imported aliases, macro-generated aliases, include-generated aliases, ambiguous aliases, and foreign machine paths are still rejected because transition introspection only follows source-backed type aliases it can resolve in-module.
  --> tests/ui/invalid_transition_result_machine_in_error_branch.rs:24:24
   |
24 |     fn finish(self) -> ::core::result::Result<(), Machine<Done>> {
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
