# statum-core

`statum-core` contains the small stable runtime surface shared by the Statum
workspace.

Public surface:

- `statum_core::Error`
- `statum_core::Result<T>`
- `StateMarker`, `UnitState`, `DataState`
- `CanTransitionTo`, `CanTransitionWith`, `CanTransitionMap`
- `statum_core::projection`

## Install

```toml
[dependencies]
statum-core = "0.5"
```

## Example

```rust
fn ensure_ready(ready: bool) -> statum_core::Result<()> {
    if ready {
        Ok(())
    } else {
        Err(statum_core::Error::InvalidState)
    }
}
```

Projection helpers are available for event-log style rebuilds:

```rust
use statum_core::projection::{ProjectionReducer, reduce_one};

struct Sum;

impl ProjectionReducer<u64> for Sum {
    type Projection = u64;
    type Error = core::convert::Infallible;

    fn seed(&self, event: &u64) -> Result<Self::Projection, Self::Error> {
        Ok(*event)
    }

    fn apply(&self, projection: &mut Self::Projection, event: &u64) -> Result<(), Self::Error> {
        *projection += event;
        Ok(())
    }
}

let total = reduce_one(vec![1_u64, 2, 3], &Sum).unwrap();
assert_eq!(total, 6);
```

## Docs

- API docs: <https://docs.rs/statum-core>
- Repository: <https://github.com/eboody/statum>
