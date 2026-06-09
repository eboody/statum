# Transition Attr Args

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_transition_attr_args.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_transition_attr_args.stderr`

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
enum WorkflowState {
    Draft,
    Review,
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

#[transition(label = "Submit")]
impl WorkflowMachine<Draft> {
    fn submit(self) -> WorkflowMachine<Review> {
        self.transition()
    }
}

fn main() {}
```

## Compiler Output

```text
error: Error: `#[transition]` does not accept arguments.
       Found: `#[transition(label = "Submit")]`
       Expected: `#[transition] impl WorkflowMachine<Draft> { ... }`
       Fix: remove the attribute arguments and declare transition behavior with methods inside the impl block.
  --> tests/ui/invalid_transition_attr_args.rs:22:1
   |
22 | #[transition(label = "Submit")]
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: this error originates in the attribute macro `transition` (in Nightly builds, run with -Z macro-backtrace for more info)
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

- Found: `#[transition(label = "Submit")]`
- Expected: `#[transition] impl WorkflowMachine<Draft> { ... }`
- Fix: remove the attribute arguments and declare transition behavior with methods inside the impl block.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
