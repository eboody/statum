# Machine Builder Reserved Field Name

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_machine_builder_reserved_field_name.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_machine_builder_reserved_field_name.stderr`

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

#[machine]
struct Workflow<WorkflowState> {
    build: String,
}

fn main() {}
```

## Compiler Output

```text
error: Error: machine `Workflow` field `build` conflicts with Statum's generated builder helper `build()`.
       Found: `build: String`
       Expected: a machine field name other than `build`
       Fix: rename that machine field before using `#[machine]`.
  --> tests/ui/invalid_machine_builder_reserved_field_name.rs:20:5
   |
20 |     build: String,
   |     ^^^^^
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

- Found: `build: String`
- Expected: a machine field name other than `build`
- Fix: rename that machine field before using `#[machine]`.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
