# Machine Generic Not First

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_machine_generic_not_first.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_machine_generic_not_first.stderr`

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
enum FooState {
    Start,
}

#[machine]
struct BadMachine<T, FooState> {
    value: T,
}
```

## Compiler Output

```text
error: Error: machine `BadMachine` uses `T` as its state generic, but the `#[state]` enum in this module is `FooState`.
       Found: `struct BadMachine<T, FooState> { ... }`
       Expected: `struct BadMachine<FooState> { ... }`
       Fix: declare `BadMachine<FooState>`.
  --> tests/ui/invalid_machine_generic_not_first.rs:20:19
   |
20 | struct BadMachine<T, FooState> {
   |                   ^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_machine_generic_not_first.rs:22:2
   |
22 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_machine_generic_not_first.rs`
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

- Found: `struct BadMachine<T, FooState> { ... }`
- Expected: `struct BadMachine<FooState> { ... }`
- Fix: declare `BadMachine<FooState>`.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
