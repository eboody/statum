# Case Study: Event Logs To Typed Machines

This is the strongest Statum example in the repo because it shows the part that
is hard to fake with normal status enums: rebuilding a workflow from
append-only events without dropping back to ad hoc runtime branching.

Source:

- runnable example:
  [statum-examples/src/showcases/sqlite_event_log_rebuild.rs](../statum-examples/src/showcases/sqlite_event_log_rebuild.rs)
- deeper validator guide: [persistence-and-validators.md](persistence-and-validators.md)
- related patterns: [patterns.md](patterns.md)

## The Problem Shape

You have an append-only event log:

- `created`
- `paid`
- `packed`
- `shipped`
- `delivered`

You need to answer questions like:

- what state is this order in now?
- which operations are legal next?
- which fields are only valid after specific events?

The usual runtime shape is:

- reduce events into a row or snapshot
- carry a status string or enum
- branch on that status later
- hope the data attached to that status is still consistent

That works, but the legal workflow still lives in runtime code.

## The Statum Shape

The example splits the problem into three explicit layers:

1. `#[state]` declares the legal phases:
   `Created`, `Paid`, `Packed`, `Shipped`, `Delivered`.
2. `statum::projection` reduces the event stream into one projection row per
   order.
3. `#[validators(OrderMachine)]` rebuilds that projection into a typed machine.

Once rebuilt, the result is not "an order plus a status field." It is one of:

- `order_machine::State::Created`
- `order_machine::State::Paid`
- `order_machine::State::Packed`
- `order_machine::State::Shipped`
- `order_machine::State::Delivered`

That matters because the workflow boundary is no longer implicit.

## What Gets Better

### 1. Illegal transitions stop being ordinary method calls

The example only defines legal edges:

- `Created -> Paid`
- `Paid -> Packed`
- `Packed -> Shipped`
- `Shipped -> Delivered`

After reconstruction, you work with the concrete typed machine. If you are in
`Packed`, the next legal operation is `ship(...)`. There is no normal method
for skipping ahead to `deliver()` from the wrong state.

### 2. State-specific data stops leaking across phases

The projection row may carry optional fields like `payment_receipt`,
`pick_ticket`, and `tracking_number`, but the rebuilt machines do not expose
them uniformly.

Instead:

- `Paid` carries `payment_receipt`
- `Packed` carries `payment_receipt` and `pick_ticket`
- `Shipped` carries `tracking_number` too

That makes the type system reflect the event history you actually observed.

### 3. Rehydration becomes one explicit boundary

Without Statum, it is common to reduce events into a snapshot and then keep
rechecking status everywhere else in the code.

In this example, projection is one step and typed rebuild is the next step:

```rust
let row = projection::reduce_one(events, &OrderProjector)?;
let state = row.into_machine().build()?;
```

After that, downstream code works on typed states rather than repeating
snapshot-to-workflow interpretation logic.

### 4. Batch rebuilds stay typed too

The same example uses `.into_machines()` after grouped projection so many
orders can be reconstructed in one pass.

That is useful because append-only systems rarely rebuild just one workflow at a
time.

## Why This Is A Good Statum Fit

This example fits Statum well because the workflow has:

- a stable set of legal phases
- expensive invalid transitions
- state-specific data that should not exist everywhere
- a persistence boundary where runtime facts must be turned back into a legal
  workflow shape

That is the kind of problem where typestate removes a real class of mistakes.

## What This Example Does Not Claim

Statum is not replacing event storage, projections, or orchestration. The
example still has an explicit projector and explicit persistence code.

What Statum owns is the workflow boundary after projection:

- are these facts a legal `Created` order?
- or `Paid`?
- or `Packed`?
- and once rebuilt, which transitions are legal next?

That is the part that would otherwise decay into scattered runtime checks.

## Where To Look Next

- [README](../README.md) for the quick mental model
- [statum-examples/src/showcases/sqlite_event_log_rebuild.rs](../statum-examples/src/showcases/sqlite_event_log_rebuild.rs)
  for the full runnable example
- [persistence-and-validators.md](persistence-and-validators.md) for the
  rebuild APIs
- [patterns.md](patterns.md) for adjacent usage patterns
