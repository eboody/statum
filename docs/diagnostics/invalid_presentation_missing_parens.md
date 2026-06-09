# Presentation Missing Parens

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_presentation_missing_parens.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_presentation_missing_parens.stderr`

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

use statum_macros::state;

#[state]
enum WorkflowState {
    #[present]
    Draft,
}
```

## Compiler Output

```text
error: Error: `#[present(...)]` on state `WorkflowState::Draft` requires parentheses.
       Found: `#[present]`
       Expected: `#[present(label = "...", description = "...")]`
       Fix: write `#[present(...)]` with key/value pairs inside the parentheses.
  --> tests/ui/invalid_presentation_missing_parens.rs:15:5
   |
15 |     #[present]
   |     ^^^^^^^^^^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_presentation_missing_parens.rs:17:2
   |
17 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_presentation_missing_parens.rs`
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

- Found: `#[present]`
- Expected: `#[present(label = "...", description = "...")]`
- Fix: write `#[present(...)]` with key/value pairs inside the parentheses.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
