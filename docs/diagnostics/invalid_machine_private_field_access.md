# Machine Private Field Access

Status: tracked compiler-fallback placeholder

Source fixture: `../../statum-macros/tests/ui/invalid_machine_private_field_access.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_machine_private_field_access.stderr`

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


mod demo {
    use super::*;

    #[state]
    pub enum LightState {
        Off,
    }

    #[machine]
    pub struct LightSwitch<LightState> {
        secret: u8,
        pub visible: u8,
    }
}

fn main() {
    let light = demo::LightSwitch::<demo::Off>::builder()
        .secret(7)
        .visible(9)
        .build();

    let _ = light.visible;
    let _ = light.secret;
}
```

## Compiler Output

```text
error[E0616]: field `secret` of struct `demo::LightSwitch` is private
  --> tests/ui/invalid_machine_private_field_access.rs:36:19
   |
36 |     let _ = light.secret;
   |                   ^^^^^^ private field
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

- This fixture intentionally records a native Rust compiler error that protects a generated surface or removed legacy API.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
