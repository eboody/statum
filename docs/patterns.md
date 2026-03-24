# Patterns and Guidance

This page collects the common Statum patterns that are useful after the quick start. The examples below are intentionally short. For executable coverage, see [../statum-examples/tests/patterns.rs](../statum-examples/tests/patterns.rs).

Each pattern is in service of the same goal: legal states should be explicit,
and undesirable states should not be smuggled through ordinary APIs.

## Branching Decisions

A transition site can expose two explicit legal targets when that branch is
part of the stable protocol surface. Use `statum::Branch<Machine<A>,
Machine<B>>` for that narrow case:

```rust
#[transition]
impl ProcessMachine<Init> {
    fn decide(
        self,
        event: Event,
    ) -> statum::Branch<ProcessMachine<NextState>, ProcessMachine<OtherState>> {
        match event {
            Event::Go => statum::Branch::First(self.transition()),
            Event::Alternative => statum::Branch::Second(self.transition()),
        }
    }
}
```

If the choice needs a richer domain enum, more than two branches, or a lot of
policy logic, keep that branching in a normal helper and dispatch into explicit
transition methods from there.

## Guarded Transitions

Keep preconditions in normal methods and transition only after the guard passes:

```rust
impl Machine<Pending> {
    fn try_activate(self) -> statum::Result<Machine<Active>> {
        if self.can_activate() {
            Ok(self.activate())
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}
```

## State-Specific Data

Attach data only to states where it is actually valid. Transition into those
states with `transition_with(data)`:

```rust
#[state]
enum ReviewState {
    Draft,
    InReview(ReviewData),
    Published,
}

#[transition]
impl Document<Draft> {
    fn submit_for_review(self, reviewer: String) -> Document<InReview> {
        self.transition_with(ReviewData { reviewer })
    }
}
```

Examples: [../statum-examples/src/toy_demos/07-state-data.rs](../statum-examples/src/toy_demos/07-state-data.rs), [../statum-examples/src/toy_demos/08-transition-with-data.rs](../statum-examples/src/toy_demos/08-transition-with-data.rs)

## Data-To-Data Edges

When the next state's payload should be derived by consuming the current
state's payload, use `transition_map(...)` instead of cloning fields into a new
value first:

```rust
#[transition]
impl Order<Packed> {
    fn ship(self, tracking: String) -> Order<Shipped> {
        self.transition_map(|packed| ShippedData {
            order_id: packed.order_id,
            tracking,
        })
    }
}
```

Example: [../statum-examples/src/toy_demos/15-transition-map.rs](../statum-examples/src/toy_demos/15-transition-map.rs)

## Async Side Effects

Keep side effects in async methods and call a synchronous transition at the end:

```rust
#[transition]
impl Job<Queued> {
    fn start(self) -> Job<Running> {
        self.transition()
    }
}

impl Job<Queued> {
    async fn start_with_effects(self) -> Job<Running> {
        do_io().await;
        self.start()
    }
}
```

Example: [../statum-examples/src/toy_demos/06-async-transitions.rs](../statum-examples/src/toy_demos/06-async-transitions.rs)

## Nested Machines

Use a machine as state data when a parent workflow owns a child workflow:

```rust
#[state]
enum ParentState {
    NotStarted,
    InProgress(SubMachine<Running>),
    Done,
}
```

Example: [../statum-examples/src/toy_demos/11-hierarchical-machines.rs](../statum-examples/src/toy_demos/11-hierarchical-machines.rs)

## Rollbacks and Snapshots

If you need undo or history, carry the prior state's data into the next state or add an explicit rollback transition:

```rust
#[transition]
impl Machine<Draft> {
    fn publish(self) -> Machine<Published> {
        let previous = self.state_data.clone();
        self.transition_with(PublishData { previous })
    }
}
```

Examples: [../statum-examples/src/toy_demos/12-rollbacks.rs](../statum-examples/src/toy_demos/12-rollbacks.rs), [../statum-examples/tests/patterns.rs](../statum-examples/tests/patterns.rs)

## Batch Rehydration With Per-Item Machine Context

Use `.into_machines_by(...)` when each persisted row needs different machine
fields during rebuild:

```rust
let machines = rows
    .into_machines_by(|row| workflow_machine::Fields {
        tenant: row.tenant.clone(),
        workflow_name: row.workflow_name.clone(),
    })
    .build();
```

Example: [../statum-examples/src/toy_demos/14-batch-machine-fields.rs](../statum-examples/src/toy_demos/14-batch-machine-fields.rs)

## Event Logs + Projection Rows

For append-only storage, project events into validator rows first and then
rehydrate typed machines:

```rust
use statum::projection::{ProjectionReducer, reduce_grouped};

let rows = reduce_grouped(events, |event| event.order_id, &OrderProjector)?;
let machines = rows.into_machines().build();
```

Example: [../statum-examples/src/showcases/sqlite_event_log_rebuild.rs](../statum-examples/src/showcases/sqlite_event_log_rebuild.rs)

## Protocol Sessions

Not every Statum workflow is persistence-driven. Session and protocol lifecycles
work well when legal frame order matters and method availability should change
by phase.

Example: [../statum-examples/src/showcases/tokio_websocket_session.rs](../statum-examples/src/showcases/tokio_websocket_session.rs)

## When To Stop

Statum works best when the stable core of a protocol is known up front. If most of your logic is runtime branching, user-authored graphs, or rapidly changing states, keep that part in normal runtime validation and use typestate only around the small stable core.
