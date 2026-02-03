# Migration Guide (Current -> New API)

This migration guide is inferred from the current macros and crate layout. It focuses on the differences implied by the code, not by the README.

## 1) `#[transition]` is now required on transition impls
Old (implicit):
```rust
impl Machine<Draft> {
    fn submit(self) -> Machine<InReview> {
        self.transition()
    }
}
```

New (explicit):
```rust
#[transition]
impl Machine<Draft> {
    fn submit(self) -> Machine<InReview> {
        self.transition()
    }
}
```

## 2) `#[validators]` attribute form changed
Old (README-style):
```rust
#[validators(state = TaskState, machine = TaskMachine)]
impl StoredTask {
    fn is_draft(&self) -> Result<()> { ... }
}
```

New (inferred from macro parsing):
```rust
#[validators(TaskMachine)]
impl StoredTask {
    fn is_draft(&self) -> Result<()> { ... }
}
```

## 3) Validators are stricter
- Must include an `is_{state}` method for every state variant (snake_case).
- Each validator must take exactly `&self` (machine fields are injected by the macro).
- Return types must match the variant:
  - unit state -> `Result<()>`
  - data state -> `Result<StateData>`
- If any validator is `async`, the generated builders become `async` too.

## 4) State enum rules are enforced
- State enums must have at least one variant.
- Variants must be unit or single-field tuple variants.
- Struct variants are rejected.
- Generics on the `#[state]` enum are not supported.

## 5) `#[machine]` generic must match the state enum name
- The first generic parameter must be the state enum name (e.g. `Machine<State>`).
- If the machine derives `Debug`, `Clone`, etc., the state enum must derive the same traits.

## 6) Constructors now use `bon` builders
The machine macro generates a per-state builder using `bon`.

Old (common style):
```rust
let m = Machine::new(...);
```

New (inferred):
```rust
let m = Machine::<Draft>::builder()
    .field_a(...)
    .build();

let m = Machine::<InReview>::builder()
    .field_a(...)
    .state_data(ReviewData { ... })
    .build();
```

You can also call the generated `new(..)` directly if you want a positional constructor.

## 7) Transition validation is type-based
- The macro no longer inspects the function body for `transition()` vs `transition_with(..)`.
- It chooses the transition trait implementation based on whether the target state carries data.

## 8) Examples moved to `statum-examples`
- Old examples under `statum/examples/*.rs` are removed.
- New examples live under `statum-examples/src/examples/`.

## Recommended Migration Order
1. Update the state enum to comply with the variant restrictions.
2. Update machine generics and derive placement.
3. Add `#[transition]` to impl blocks.
4. Update transitions to use `transition()` / `transition_with(..)` correctly.
5. Update validators to the new attribute form and per-variant requirements.
6. Switch construction to the generated builders.
7. Re-run `cargo test -p statum-macros` to confirm UI diagnostics.

