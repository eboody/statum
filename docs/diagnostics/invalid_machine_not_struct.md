# Machine Not Struct

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_machine_not_struct.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_machine_not_struct.stderr`

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

use statum_macros::machine;


#[machine]
enum NotAStruct {
    Variant,
}
```

## Compiler Output

```text
error: Error: #[machine] must be applied to a struct.
       Found: `enum NotAStruct { ... }`
       Expected: `struct NotAStruct<WorkflowState> { ... }`
       Fix: change `NotAStruct` from an enum into a `#[machine]` struct whose first generic names the local `#[state]` enum.
  --> tests/ui/invalid_machine_not_struct.rs:15:6
   |
15 | enum NotAStruct {
   |      ^^^^^^^^^^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_machine_not_struct.rs:17:2
   |
17 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_machine_not_struct.rs`
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

- Found: `enum NotAStruct { ... }`
- Expected: `struct NotAStruct<WorkflowState> { ... }`
- Fix: change `NotAStruct` from an enum into a `#[machine]` struct whose first generic names the local `#[state]` enum.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
