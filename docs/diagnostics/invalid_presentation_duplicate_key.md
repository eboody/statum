# Presentation Duplicate Key

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_presentation_duplicate_key.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_presentation_duplicate_key.stderr`

## Broken Example

```rust
#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state};

#[state]
enum FlowState {
    Draft,
}

#[machine]
#[present(label = "Flow", label = "Again")]
struct Flow<FlowState> {}

fn main() {}
```

## Compiler Output

```text
error: Error: duplicate `#[present(...)]` key `label` on machine `Flow`.
       Found: `label = "Again"`
       Expected: one `label` presentation field entry
       Fix: specify `label` at most once inside `#[present(...)]`.
  --> tests/ui/invalid_presentation_duplicate_key.rs:19:27
   |
19 | #[present(label = "Flow", label = "Again")]
   |                           ^^^^^
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

- Found: `label = "Again"`
- Expected: one `label` presentation field entry
- Fix: specify `label` at most once inside `#[present(...)]`.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
