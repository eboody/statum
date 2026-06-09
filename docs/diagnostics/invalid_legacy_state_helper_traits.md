# Legacy State Helper Traits

Status: tracked compiler-fallback placeholder

Source fixture: `../../statum-macros/tests/ui/invalid_legacy_state_helper_traits.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_legacy_state_helper_traits.stderr`

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

#[state]
enum TaskState {
    Draft,
    Review(String),
}

fn assert_state_variant<T: StateVariant>() {}

fn assert_requires_state_data<T: RequiresStateData>() {}

fn assert_does_not_require_state_data<T: DoesNotRequireStateData>() {}

fn main() {
    assert_state_variant::<Draft>();
    assert_requires_state_data::<Review>();
    assert_does_not_require_state_data::<Draft>();
}
```

## Compiler Output

```text
error[E0405]: cannot find trait `StateVariant` in this scope
  --> tests/ui/invalid_legacy_state_helper_traits.rs:21:28
   |
21 | fn assert_state_variant<T: StateVariant>() {}
   |                            ^^^^^^^^^^^^ not found in this scope

error[E0405]: cannot find trait `RequiresStateData` in this scope
  --> tests/ui/invalid_legacy_state_helper_traits.rs:23:34
   |
23 | fn assert_requires_state_data<T: RequiresStateData>() {}
   |                                  ^^^^^^^^^^^^^^^^^ not found in this scope

error[E0405]: cannot find trait `DoesNotRequireStateData` in this scope
  --> tests/ui/invalid_legacy_state_helper_traits.rs:25:42
   |
25 | fn assert_does_not_require_state_data<T: DoesNotRequireStateData>() {}
   |                                          ^^^^^^^^^^^^^^^^^^^^^^^ not found in this scope
```

## Corrected Example

```rust
// This fixture is tracked as a compiler-regression placeholder.
// Keep the invalid test, and prefer a nearby valid UI fixture for the corrected shape.
```

## Explanation

- This fixture intentionally records a native Rust compiler error that protects a generated surface or removed legacy API.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
