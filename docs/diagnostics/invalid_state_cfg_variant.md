# State Cfg Variant

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_state_cfg_variant.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_state_cfg_variant.stderr`

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
    Draft,
    #[cfg(any())]
    Hidden,
}
```

## Compiler Output

```text
error: Error: `#[state]` enum `WorkflowState` variant `Hidden` uses `#[cfg]`, but Statum does not support conditionally compiled state variants.
       Found: `#[cfg(any())] Hidden`
       Expected: an unconditional `Hidden` variant inside `WorkflowState`
       Fix: move the cfg gate to the whole `#[state]` enum or split cfg-specific workflows into separate modules.
  --> tests/ui/invalid_state_cfg_variant.rs:15:5
   |
15 | /     #[cfg(any())]
16 | |     Hidden,
   | |__________^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_state_cfg_variant.rs:17:2
   |
17 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_state_cfg_variant.rs`
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

- Found: `#[cfg(any())] Hidden`
- Expected: an unconditional `Hidden` variant inside `WorkflowState`
- Fix: move the cfg gate to the whole `#[state]` enum or split cfg-specific workflows into separate modules.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
