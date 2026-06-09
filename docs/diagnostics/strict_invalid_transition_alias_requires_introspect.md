# Transition Alias Requires Introspect

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/strict_invalid_transition_alias_requires_introspect.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/strict_invalid_transition_alias_requires_introspect.stderr`

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

type Next =
    ::core::result::Result<WorkflowMachine<Review>, WorkflowMachine<Rejected>>;

#[state]
enum WorkflowState {
    Draft,
    Review,
    Rejected,
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

#[transition]
impl WorkflowMachine<Draft> {
    fn submit(self, approve: bool) -> Next {
        if approve {
            Ok(self.review())
        } else {
            Err(self.reject())
        }
    }

    fn review(self) -> WorkflowMachine<Review> {
        self.transition()
    }

    fn reject(self) -> WorkflowMachine<Rejected> {
        self.transition()
    }
}

fn main() {}
```

## Compiler Output

```text
error: Error: transition method `WorkflowMachine<Draft>::submit` returns an unsupported type.
       Found: `fn submit(self) -> Next`
       Expected: `fn submit(self) -> WorkflowMachine<NextState>`
       Reason: expected the impl target machine path directly, or that same machine path wrapped in a supported `Option`, `Result`, or `Branch` shape; aliases require an explicit `#[introspect(return = ...)]` annotation in strict mode
       Fix: return `WorkflowMachine<NextState>` directly.
       Primary branch: `Next`
       Note: Supported strict introspection shapes are direct machine paths and supported `::core::option::Option<...>`, `::core::result::Result<..., E>`, and `::statum::Branch<..., ...>` wrappers around direct machine paths.
             Source-backed aliases may be expanded only to suggest an explicit `#[introspect(return = ...)]`; they are not accepted as authoritative transition contracts in strict mode. Imported aliases, macro-generated aliases, include-generated aliases, ambiguous aliases, and foreign machine paths are rejected.
       Help: add `#[introspect(return = ::core::result::Result<WorkflowMachine<Review>, WorkflowMachine <
             Rejected>>)]` to this method, or rewrite the signature to use that direct type.
             Source-backed alias expansion is diagnostics-only in strict mode.
  --> tests/ui/strict_invalid_transition_alias_requires_introspect.rs:28:39
   |
28 |     fn submit(self, approve: bool) -> Next {
   |                                       ^^^^
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

- Found: `fn submit(self) -> Next`
- Expected: `fn submit(self) -> WorkflowMachine<NextState>`
- Fix: return `WorkflowMachine<NextState>` directly.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
