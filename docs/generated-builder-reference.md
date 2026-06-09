# Generated Builder Reference

This page documents the builder surface that Statum generates for typed machines
and validator-based rebuilds. It describes the intended user-facing API shape,
not the hidden marker types or generated storage field names that may appear in
compiler diagnostics.

For product positioning, see [builder-ux-positioning.md](builder-ux-positioning.md).
For deciding whether a typestate shape is useful at all, see
[typestate-builder-design-playbook.md](typestate-builder-design-playbook.md).

## Initial Machine Builders

`#[machine]` generates a builder for each concrete state marker produced by the
`#[state]` enum:

```rust
let draft = DocumentMachine::<Draft>::builder()
    .id("doc-1".to_owned())
    .title("Draft title".to_owned())
    .build();
```

Use these builders when you already know the starting state and want to create a
new typed machine value. They are not general-purpose replacement builders for
request DTOs, config structs, optional defaults, or arbitrary validation hooks.
Let ordinary builder crates assemble those inputs, then pass the resulting data
into a typed machine.

## Required Inputs

Every machine field is required. A builder exposes one setter with the same name
as each machine field:

```rust
#[machine]
struct DocumentMachine<DocumentState> {
    id: String,
    title: String,
}

let draft = DocumentMachine::<Draft>::builder()
    .id("doc-1".to_owned())
    .title("Draft title".to_owned())
    .build();
```

`build()` is only available after all required setters for that state have been
called. If a setter is missing, the Rust compiler reports that `build` does not
exist for the current builder state. Read the generated marker suffix in the
error to find the missing slot, but do not depend on the full generated marker
name as a public contract.

Setters consume the builder and return the next builder state. Calling the same
setter twice is rejected because the second builder state no longer exposes that
setter:

```rust
let draft = DocumentMachine::<Draft>::builder()
    .title("first".to_owned())
    .title("second".to_owned()); // compile error: setter already used
```

When this happens, keep one setter call and merge duplicate inputs before the
builder chain.

## Data-Bearing States And `state_data`

If the target state variant carries data, that payload is another required
input named `state_data`:

```rust
#[state]
enum DocumentState {
    Draft,
    InReview(ReviewAssignment),
}

let in_review = DocumentMachine::<InReview>::builder()
    .id("doc-1".to_owned())
    .title("Draft title".to_owned())
    .state_data(ReviewAssignment {
        reviewer: "Ada".to_owned(),
    })
    .build();
```

Unit states do not expose `state_data`. Data-bearing states require exactly one
`state_data(...)` call before `build()` is available.

Use this split deliberately:

- Machine fields are context that remains valid across states.
- `state_data` is payload that is only valid for the target state.

If a value is needed in many states, prefer a machine field. If a value only
makes sense in one phase, prefer state data.

## Visibility

Generated builders follow the machine struct's visibility. A public machine gets
a public `builder()` method and public builder methods. A private or restricted
machine keeps its generated builders at the same visibility boundary.

This makes the generated construction surface match the machine surface instead
of leaking private workflows across module boundaries.

## Generics, Lifetimes, And Where Clauses

Machine generics, lifetimes, const parameters, and where clauses are carried onto
the generated builder surface:

```rust
#[machine]
pub struct JobMachine<'a, T, JobState>
where
    T: Clone,
{
    tenant: &'a str,
    payload: T,
}

let queued = JobMachine::<Payload, Queued>::builder()
    .tenant("acme")
    .payload(payload)
    .build();
```

Keep generic constraints on the machine definition when callers need the same
constraints to construct that machine. If rust-analyzer shows a simpler builder
shape while editing, treat it as an IDE fallback; compile with `cargo check` or
the macro test suite before relying on diagnostics.

## Rebuild Builders From `#[validators]`

`#[validators(Machine)]` generates builders for reconstructing typed machines
from persisted or external records. For a single row, prefer the type-first entry
point:

```rust
let rebuilt = DocumentMachine::rebuild(&row)
    .tenant_id(tenant_id)
    .build()?;
```

`row.into_machine()` remains available and produces the same kind of builder.
Both forms require every machine field before `build()` or `build_report()` is
available. State payloads are produced by validator methods; callers do not pass
`state_data` to rebuild builders.

For collections where every row shares the same machine fields, use
`rebuild_many` or `into_machines`:

```rust
let machines = DocumentMachine::rebuild_many(rows)
    .tenant_id(tenant_id.clone())
    .build();
```

Shared-field batch builders clone supplied machine fields for each item, so those
field types must implement `Clone`. When machine fields vary per row, use the
per-item field callback instead:

```rust
let machines = rows.into_machines_by(|row| document_machine::Fields {
    tenant_id: row.tenant_id.clone(),
});
```

If any validator is async, rebuild finalizers are async too; call `.await` on
`build()` or `build_report()`.

## Collisions And Reserved Names

Generated builders use machine field names as setter names. Field identifiers
therefore need to coexist with generated methods and helper names.

Known collision rules:

- A machine field named `state_data` conflicts with data-bearing state builders,
  because `state_data(...)` is reserved for the target state's payload.
- A machine field with the same name as a generated helper can be rejected during
  macro expansion.
- Raw identifiers such as `r#type` are supported as setters with the same raw
  identifier spelling.
- Similar-looking names such as `foo_bar` and `foo__bar` are treated as distinct
  Rust identifiers, but diagnostics may include normalized marker suffixes. Use
  the original field names in your source as the authority.

When a collision error appears, rename the machine field or move that value into
state data if it is phase-specific.

## What Is Not A Public Contract

These details are intentionally not stable API:

- hidden marker type names used to track missing and set builder slots,
- slot numbers in compiler diagnostics,
- generated storage field names,
- hidden rebuild-builder struct names,
- rust-analyzer-only fallback builder internals.

The stable surface to rely on is the macro input plus the visible generated
methods: `Machine::<State>::builder()`, field setters, `state_data(...)` for
payload states, `build()`, rebuild entrypoints, and rebuild report finalizers.

## Troubleshooting Checklist

- `build()` is missing: one or more required machine fields, or `state_data` for
  a data-bearing state, has not been supplied.
- A setter is missing after you called it once: the duplicate call is rejected by
  the typestate builder state.
- `state_data(...)` is missing: the target state is a unit state, or you are using
  a validator rebuild builder where validators produce state payloads.
- Batch rebuild asks for `Clone`: use shared fields only for cloneable machine
  context, or switch to `into_machines_by` for per-row fields.
- A generated name collides with a field: rename the field or reconsider whether
  it belongs as phase-specific state data.
