# Machine Unknown Attr Key

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_machine_unknown_attr_key.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_machine_unknown_attr_key.stderr`

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

#[state]
enum WorkflowState {
    Draft,
}

#[machine(start = Draft)]
struct Workflow<WorkflowState> {
    id: u64,
}

fn main() {}
```

## Compiler Output

```text
error: Error: `#[machine]` does not accept arguments.
       Found: `#[machine(start = Draft)]`
       Expected: `#[machine] struct WorkflowMachine<WorkflowState> { ... }`
       Fix: remove the attribute arguments and link the machine to `#[state]` through its first generic parameter.
  --> tests/ui/invalid_machine_unknown_attr_key.rs:18:1
   |
18 | #[machine(start = Draft)]
   | ^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: this error originates in the attribute macro `machine` (in Nightly builds, run with -Z macro-backtrace for more info)
```

## Corrected Example

```rust
use statum::{machine, state};

#[state]
enum WorkflowState {
    Draft(DraftData),
}

struct DraftData {
    name: String,
}

#[machine]
struct WorkflowMachine<WorkflowState> {
    owner: String,
}

let machine = WorkflowMachine::draft_builder()
    .owner("ops".to_string())
    .state_data(DraftData { name: "doc".to_string() })
    .build();
```

## Explanation

- Found: `#[machine(start = Draft)]`
- Expected: `#[machine] struct WorkflowMachine<WorkflowState> { ... }`
- Fix: remove the attribute arguments and link the machine to `#[state]` through its first generic parameter.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
