# Patterns and Guidance

This page collects the common Statum patterns that are useful after the quick start. The examples below are intentionally short. For executable coverage, see [../statum-examples/tests/patterns.rs](../statum-examples/tests/patterns.rs).

## Branching Decisions

A transition method should still return one concrete next state. Put branching in a normal helper and dispatch into explicit transition methods:

```rust
enum Decision {
    Next(ProcessMachine<NextState>),
    Other(ProcessMachine<OtherState>),
}

impl ProcessMachine<Init> {
    fn decide(self, event: Event) -> Decision {
        match event {
            Event::Go => Decision::Next(self.to_next()),
            Event::Alternative => Decision::Other(self.to_other()),
        }
    }
}
```

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

Attach data only to states where it is actually valid. Transition into those states with `transition_with(data)`:

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

## When To Stop

Statum works best when the stable core of a protocol is known up front. If most of your logic is runtime branching, user-authored graphs, or rapidly changing states, keep that part in normal runtime validation and use typestate only around the small stable core.
