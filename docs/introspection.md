# Machine Introspection

Statum can emit typed machine introspection directly from the machine
definition itself.

Use it when the machine definition should also drive downstream tooling:

- CLI explainers
- generated docs
- graph exports
- branch-strip views
- test assertions about exact legal transitions
- replay or debug tooling that joins runtime events back to the static graph

The important distinction is precision. Statum does not only expose a
machine-wide list of states. It exposes exact transition sites:

- source state
- transition method
- exact legal target states from that site

That means a branching transition like `Flow<Fetched>::validate() ->
Accepted | Rejected` can be rendered without maintaining a parallel handwritten
graph table.

## Static Graph Access

Use `MachineIntrospection` to get the generated graph:

```rust
use statum::{machine, state, transition, MachineIntrospection};

#[state]
enum FlowState {
    Fetched,
    Accepted,
    Rejected,
}

#[machine]
struct Flow<FlowState> {}

#[transition]
impl Flow<Fetched> {
    fn validate(self, accept: bool) -> Result<Flow<Accepted>, Flow<Rejected>> {
        if accept {
            Ok(self.accept())
        } else {
            Err(self.reject())
        }
    }

    fn accept(self) -> Flow<Accepted> {
        self.transition()
    }

    fn reject(self) -> Flow<Rejected> {
        self.transition()
    }
}

let graph = <Flow<Fetched> as MachineIntrospection>::GRAPH;
let validate = graph
    .transition_from_method(flow::StateId::Fetched, "validate")
    .unwrap();

assert_eq!(
    graph.legal_targets(validate.id).unwrap(),
    &[flow::StateId::Accepted, flow::StateId::Rejected]
);
```

From there, a consumer can ask for:

- transitions from a state
- a transition by id
- a transition by source state and method name
- the exact legal targets for a transition site

## Transition Identity

State ids are generated as a machine-scoped enum like `flow::StateId`.

Transition ids are typed and exact, but they are exposed as generated
associated consts on the source-state machine type, such as
`Flow::<Fetched>::VALIDATE`.

That keeps transition identity tied to the exact source-state plus method site,
including cfg-pruned and macro-generated transitions.

## Runtime Join Support

If you want replay or debug tooling, record the transition that actually
happened at runtime and join it back to the static graph:

```rust
use statum::{
    machine, state, transition, MachineTransitionRecorder,
};

#[state]
enum FlowState {
    Fetched,
    Accepted,
    Rejected,
}

#[machine]
struct Flow<FlowState> {}

#[transition]
impl Flow<Fetched> {
    fn validate(self, accept: bool) -> Result<Flow<Accepted>, Flow<Rejected>> {
        if accept {
            Ok(self.accept())
        } else {
            Err(self.reject())
        }
    }

    fn accept(self) -> Flow<Accepted> {
        self.transition()
    }

    fn reject(self) -> Flow<Rejected> {
        self.transition()
    }
}

let event = <Flow<Fetched> as MachineTransitionRecorder>::try_record_transition_to::<
    Flow<Accepted>,
>(Flow::<Fetched>::VALIDATE)
.unwrap();

assert_eq!(event.chosen, flow::StateId::Accepted);
```

Once you have both:

- static graph metadata
- runtime-taken transition records

you can render the chosen branch and the non-chosen legal branches from the
same source of truth.

## Presentation Metadata

Structural introspection is separate from human-facing metadata.

If a consumer crate wants labels, descriptions, or phases for rendering, it can
add a typed `MachinePresentation` overlay keyed by the generated ids. That lets
the machine definition remain the source of truth for structure while the
consumer owns local explanation and presentation.

## Example

Runnable example:
[statum-examples/src/toy_demos/16-machine-introspection.rs](../statum-examples/src/toy_demos/16-machine-introspection.rs)
