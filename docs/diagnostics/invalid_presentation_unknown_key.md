# Presentation Unknown Key

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_presentation_unknown_key.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_presentation_unknown_key.stderr`

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

use statum_macros::{machine, state, transition};

#[state]
enum FlowState {
    Draft,
    Review,
}

#[machine]
struct Flow<FlowState> {}

#[transition]
impl Flow<Draft> {
    #[present(title = "Submit")]
    fn submit(self) -> Flow<Review> {
        self.transition()
    }
}

fn main() {}
```

## Compiler Output

```text
error: Error: unknown `#[present(...)]` key `title` on transition `Flow<Draft>::submit`.
       Found: `title = "Submit"`
       Expected: `label = "..."`, `description = "..."`, or `metadata = Expr`
       Fix: replace that key or remove it.
  --> tests/ui/invalid_presentation_unknown_key.rs:24:15
   |
24 |     #[present(title = "Submit")]
   |               ^^^^^
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

- Found: `title = "Submit"`
- Expected: `label = "..."`, `description = "..."`, or `metadata = Expr`
- Fix: replace that key or remove it.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
