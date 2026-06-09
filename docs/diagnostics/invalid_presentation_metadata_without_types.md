# Presentation Metadata Without Types

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_presentation_metadata_without_types.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_presentation_metadata_without_types.stderr`

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

use statum_macros::{machine, state};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FlowStateMeta {
    Draft,
}

#[state]
enum FlowState {
    #[present(label = "Draft", metadata = FlowStateMeta::Draft)]
    Draft,
}

#[machine]
struct Flow<FlowState> {}

fn main() {}
```

## Compiler Output

```text
error: Error: state `FlowState::Draft` uses `#[present(metadata = ...)]`, but machine `Flow` did not declare `#[presentation_types(state = ...)]`.
       Found: `#[present(metadata = FlowStateMeta :: Draft)]`
       Expected: `#[presentation_types(state = StateMeta)]` on machine `Flow`
       Fix: add `#[presentation_types(state = StateMeta)]` to the `#[machine]` struct or remove the metadata expression.
  --> tests/ui/invalid_presentation_metadata_without_types.rs:24:1
   |
24 | #[machine]
   | ^^^^^^^^^^
   |
   = note: this error originates in the attribute macro `machine` (in Nightly builds, run with -Z macro-backtrace for more info)
```

## Corrected Example

```rust
use statum::{machine, state};

#[state]
enum WorkflowState {
    #[present(label = "Draft", description = "Waiting for edits")]
    Draft,
}

#[machine]
#[presentation_types(state = WorkflowStatePresentation)]
struct WorkflowMachine<WorkflowState> {}

enum WorkflowStatePresentation {
    Draft,
}
```

## Explanation

- Found: `#[present(metadata = FlowStateMeta :: Draft)]`
- Expected: `#[presentation_types(state = StateMeta)]` on machine `Flow`
- Fix: add `#[presentation_types(state = StateMeta)]` to the `#[machine]` struct or remove the metadata expression.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
