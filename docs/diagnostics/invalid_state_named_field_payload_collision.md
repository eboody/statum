# State Named Field Payload Collision

Status: tracked compiler-fallback placeholder

Source fixture: `../../statum-macros/tests/ui/invalid_state_named_field_payload_collision.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_state_named_field_payload_collision.stderr`

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
use statum_macros::state;

pub struct DraftData;

#[state]
enum BadState {
    Draft { version: u32 },
}

fn main() {}
```

## Compiler Output

```text
error[E0428]: the name `DraftData` is defined multiple times
  --> tests/ui/invalid_state_named_field_payload_collision.rs:14:1
   |
12 | pub struct DraftData;
   | --------------------- previous definition of the type `DraftData` here
13 |
14 | #[state]
   | ^^^^^^^^ `DraftData` redefined here
   |
   = note: `DraftData` must be defined only once in the type namespace of this module
   = note: this error originates in the attribute macro `state` (in Nightly builds, run with -Z macro-backtrace for more info)
```

## Corrected Example

```rust
use statum::state;

#[state]
enum WorkflowState {
    Draft,
    Review(ReviewData),
}

struct ReviewData {
    priority: u8,
}
```

## Explanation

- This fixture intentionally records a native Rust compiler error that protects a generated surface or removed legacy API.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
