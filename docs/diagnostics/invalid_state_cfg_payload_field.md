# State Cfg Payload Field

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_state_cfg_payload_field.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_state_cfg_payload_field.stderr`

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
    Review {
        reviewer: &'static str,
        #[cfg_attr(any(), allow(dead_code))]
        priority: u8,
    },
}
```

## Compiler Output

```text
error: Error: `#[state]` enum `WorkflowState` variant `Review` field `priority` uses `#[cfg_attr]`, but Statum does not support conditionally compiled state payload fields.
       Found: `#[cfg_attr(any(), allow(dead_code))] priority: u8`
       Expected: an unconditional payload field for `Review`
       Fix: move the cfg gate to the whole variant or wrap the cfg-specific payload shape behind a separate type.
  --> tests/ui/invalid_state_cfg_payload_field.rs:16:9
   |
16 | /         #[cfg_attr(any(), allow(dead_code))]
17 | |         priority: u8,
   | |____________________^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_state_cfg_payload_field.rs:19:2
   |
19 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_state_cfg_payload_field.rs`
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

- Found: `#[cfg_attr(any(), allow(dead_code))] priority: u8`
- Expected: an unconditional payload field for `Review`
- Fix: move the cfg gate to the whole variant or wrap the cfg-specific payload shape behind a separate type.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
