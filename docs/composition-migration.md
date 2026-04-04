# Composition Machine Migration

This guide is for codebases that already use exact relation export,
`#[via(...)]`, `*_and_attest()`, or `#[machine_ref(...)]`, but want the
inspector to derive the main workspace story from typed orchestration instead
of external narrative scaffolding.

The target model is simple:

- leaf protocol legality stays in ordinary machines
- cross-machine orchestration moves into `#[machine(role = composition)]`
- direct child-machine values are the default exact composition surface
- detached artifacts keep exact provenance through `*_and_attest()` plus
  `#[via(...)]`

## 1. Start With A Composition Machine

If one business flow spans multiple machines and that orchestration is real
protocol truth, model it directly:

```rust
use statum::{machine, state, transition};

#[state]
enum DocumentFlowState {
    Draft,
    Reviewing(review::Machine<review::Pending>),
    Approved(review::Machine<review::Approved>),
}

#[machine(role = composition)]
struct DocumentFlow<DocumentFlowState> {
    document_id: u64,
}

#[transition]
impl DocumentFlow<Draft> {
    fn submit_for_review(
        self,
        review: review::Machine<review::Pending>,
    ) -> DocumentFlow<Reviewing> {
        self.transition_with(review)
    }
}
```

That one machine now defines the top-level workspace flow. The inspector can
open on it directly instead of making the user reconstruct the story from a
raw machine list.

Runnable example:
[statum-examples/src/toy_demos/example_18_composition_machine.rs](../statum-examples/src/toy_demos/example_18_composition_machine.rs)

## 2. Keep Direct Child Machines As The Default

When the composition state or transition can hold the child machine itself,
use that. It gives Statum the strongest exact surface with the least API.

Good fit:

- parent flow enters review and stores `review::Machine<review::Pending>`
- parent flow records approval by consuming `review::Machine<review::Approved>`
- parent flow owns a direct child machine state as part of its own legality

Do not add `#[via(...)]` or `#[machine_ref(...)]` when the child machine value
already expresses the boundary honestly.

## 3. Keep `#[via(...)]` For Detached Artifacts

When the boundary is not the child machine value itself, keep the provenance on
the detached handoff:

```rust
#[transition]
impl DocumentFlow<Approved> {
    fn record_publication(
        self,
        #[via(self::publication::machine::via::Publish)]
        publication: publication::Machine<publication::Published>,
    ) -> DocumentFlow<Published> {
        let _ = publication;
        self.transition()
    }
}
```

Producer side:

```rust
let published = publication::Machine::<publication::Ready>::builder()
    .document_id(7)
    .build()
    .publish_and_attest();

let document = document.from_publish(published).record_publication();
```

The composition machine still owns the main journey. `#[via(...)]` stays as the
exact evidence layer underneath it.

Lower-level example:
[statum-examples/src/toy_demos/17-attested-composition.rs](../statum-examples/src/toy_demos/17-attested-composition.rs)

Composition-first example:
[statum-examples/src/toy_demos/example_18_composition_machine.rs](../statum-examples/src/toy_demos/example_18_composition_machine.rs)

## 4. Keep `#[machine_ref(...)]` For Opaque References

Use `#[machine_ref(...)]` only when the cross-machine boundary is a stable
nominal type that points at another machine state without carrying the machine
value or one exact producer transition.

Good fit:

- persisted ids
- durable handoff keys
- wrapper types that represent a machine-state reference but are not emitted by
  one exact transition site

Do not use `#[machine_ref(...)]` when a direct child machine or `#[via(...)]`
already says enough.

## 5. Delete Old Narrative Overlays

If your workspace previously used hand-written journey metadata or other
inspector-only narrative overlays, remove them once the composition machines
and detached-handoff surfaces are in place.

The target state is one source of workspace-flow truth:

- composition machines for cross-machine legality
- direct child machines where possible
- `#[via(...)]` only where detached exact provenance still matters
- `#[machine_ref(...)]` only for honest opaque references

## 6. Validate The Migration

Use the tooling in this order:

1. `cargo statum-graph suggest /path/to/workspace`
   - find typed orchestration that should likely become
     `#[machine(role = composition)]`
2. `cargo statum-graph export /path/to/workspace`
   - verify the exact bundle now shows composition-owned workflow edges
3. `cargo statum-graph inspect /path/to/workspace`
   - verify `Composition` becomes the useful top-level home view

If the top-level flow still appears only in heuristics, the orchestration is
still hidden in bodies, helpers, services, or external wiring instead of typed
composition state and transition surfaces.
