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

The graph is derived from macro-expanded, cfg-pruned `#[transition]` method
signatures and supported wrapper shapes. Today that means direct
`Machine<NextState>` returns plus canonical wrapper paths around those machine
types: `::core::option::Option<...>`, `::core::result::Result<..., E>`, and
`::statum::Branch<..., ...>`. Unsupported custom decision enums, wrapper
aliases, and differently-qualified machine paths are rejected instead of
approximated. Whole-item `#[cfg]` gates are supported, but nested `#[cfg]` or
`#[cfg_attr]` on `#[state]` variants, variant payload fields, or `#[machine]`
fields are rejected because they would otherwise drift the generated metadata
from the active build.

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
    fn validate(
        self,
        accept: bool,
    ) -> ::core::result::Result<Flow<Accepted>, Flow<Rejected>> {
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

If you want a ready-made static graph export instead of writing your own
renderer, `statum-graph` builds validated `MachineDoc` values from this graph
surface, joins optional presentation labels and descriptions, and renders
Mermaid, DOT, PlantUML, or stable JSON output.

For a linked-build codebase view, `statum-graph::CodebaseDoc::linked()` also
collects every linked compiled machine family and exports:

- legacy direct payload links from state data
- declared validator-entry surfaces from compiled `#[validators]` impls
- direct-construction availability per state
- exact relation records from state payloads, machine fields, transition
  parameters, and nominal `#[machine_ref(...)]` declarations

That combined view is still static. It is not a whole-workspace source scan, it
does not model runtime orchestration, and validator entries describe declared
rebuild surfaces rather than runtime match outcomes. Validator node labels use
the impl self type as written in source, so they are human-facing display
syntax rather than canonical Rust type identity. Method-level `#[cfg]` and
`#[cfg_attr]` on validator methods are rejected at the macro layer, so the
linked validator inventory covers only supported compiled validator impl
shapes. Validator impls inside `include!()` files are also rejected at the
macro layer. In v1, exact direct-type relations recurse only through canonical
absolute carrier paths such as `::core::option::Option<...>` and
`::core::result::Result<..., E>`, and direct machine targets must use explicit
`crate::`, `self::`, `super::`, or absolute paths rather than imported aliases
or bare prelude names. `#[machine_ref(...)]` is trait-backed and supports
nominal structs and tuple structs only; plain type aliases are rejected.

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
    fn validate(
        self,
        accept: bool,
    ) -> ::core::result::Result<Flow<Accepted>, Flow<Rejected>> {
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

For lighter-weight cases, Statum can also emit a generated
`machine::PRESENTATION` constant from source-local attributes:

- `#[present(label = "...", description = "...")]` on the machine, state
  variants, and transition methods
- `#[presentation_types(machine = ..., state = ..., transition = ...)]` on the
  machine when you want typed `metadata = ...` payloads in the generated
  presentation surface

Typed presentation metadata follows the same observation point as the graph:
macro-expanded, cfg-pruned items and supported attribute shapes. If a category
declares `#[presentation_types(...)]`, each annotated item in that category
must supply `metadata = ...`; otherwise the macro rejects it instead of
guessing a default value.

`statum-graph` can join those labels and descriptions onto its stable
`ExportDoc` surface. The built-in JSON renderer keeps arbitrary typed
`metadata` out of the default output so the exported format stays deterministic
without requiring every metadata type to be serializable.

For the codebase surface, the same linked compiled observation point applies.
Machine-local topology comes from the generated machine graph and transition
inventory. Static cross-machine links come only from direct machine-like
payload types written in state data. Resolution uses normalized path suffixes
plus target state names and fails closed on ambiguity instead of guessing.

## Example

Runnable example:
[statum-examples/src/toy_demos/16-machine-introspection.rs](../statum-examples/src/toy_demos/16-machine-introspection.rs)
