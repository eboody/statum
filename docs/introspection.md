# Machine Introspection

Statum can emit typed machine introspection directly from the machine
definition itself.

Use it when the machine definition should also drive downstream tooling:

- CLI explainers
- generated docs
- graph exports
- branch-strip views
- test assertions about strict-mode legal transitions
- replay or debug tooling that joins runtime events back to the static graph
- future agent resources, such as a read-only MCP protocol metadata view

The important distinction is precision. Statum does not only expose a
machine-wide list of states. For supported syntax, strict mode exposes
transition sites scoped to the active macro input:

- source state
- transition method
- legal target states from that site

That means a branching transition like `Flow<Fetched>::validate() ->
Accepted | Rejected` can be rendered without maintaining a parallel handwritten
graph table.

The graph is derived from the cfg-pruned `#[transition]` item input visible to
Statum's attribute macros: directly readable method signatures plus supported
wrapper shapes. Today that means direct
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
- the strict-mode legal targets for a transition site

## Transition Identity

State ids are generated as a machine-scoped enum like `flow::StateId`.

Transition ids are typed and scoped to one source-state method site, but they
are exposed as generated associated consts on the source-state machine type,
such as `Flow::<Fetched>::VALIDATE`.

That keeps transition identity tied to the specific source-state plus method
site visible in the active cfg-pruned macro input. Transition sites generated
outside that attribute input are not part of the strict authority claim unless
they are made explicit through supported syntax before Statum observes them.

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
same generated metadata boundary.

## Stable Graph Metadata

`StableGraphMetadata::from_graph(...)` lowers the typed graph into a stable Rust
and JSON shape for tooling. Version 1 serializes these top-level keys:

- `version`: currently `"v1"`
- `authority`: currently `"cfg_pruned_macro_input"`
- `unsupported_cases`: explicit omissions and rejected cases
- `machine`: module path, Rust type path, optional label/description, and a
  reserved `fields` array
- `states`: Rust state names, optional label/description, payload flag, and a
  reserved `fields` array
- `transitions`: method name, optional label/description, source state, and
  strict-mode legal target states

The claimed authority surface is the graph Statum emits for supported macro
input shapes in the active cfg-pruned build. The actual observation point is the
cfg-pruned attribute macro input plus the supported return-type wrappers listed
above. It does not observe arbitrary function bodies, runtime-only transition
choices, or type-checked Rust semantics outside that macro input.

For the full surface-by-surface boundary, including raw source, parsed AST,
expanded items, runtime registry values, and persisted state, see
[introspection-authority.md](introspection-authority.md).

Unsupported cases are part of the model, not an afterthought. Version 1 declares
runtime-only transitions, unexpanded custom decision enums, and field-level
presentation metadata as unsupported. The JSON keeps reserved `fields` arrays so
consumers can depend on the shape without pretending those fields are populated
today.

The same metadata can render deterministic text graph artifacts without changing
its authority surface. `to_mermaid_state_diagram()` emits Mermaid
`stateDiagram-v2`; `to_dot_graph()` emits Graphviz DOT with stable node ids based
on graph order; `to_transition_matrix_table()` emits a Markdown table whose rows
are source states, columns are target states, and cells are transition labels or
`forbidden`. These renderers operate only on `StableGraphMetadata`, preserve
unknown transition targets as explicit placeholder nodes in graph diagrams, and
escape labels so CI jobs can diff committed artifacts byte-for-byte.

The canonical `cargo statum graph --machine axum-sqlite-review --format matrix`
output is:

```markdown
| from \\ to | Draft | InReview | Published |
| --- | --- | --- | --- |
| Draft | forbidden | submit | forbidden |
| InReview | forbidden | forbidden | approve |
| Published | forbidden | forbidden | forbidden |
```

`cargo statum graph --machine axum-sqlite-review --format lints` runs the
prototype graph invariant lints over the same stable metadata. The first lint
set checks for unreachable states from the first graph-order state, non-initial
states with no incoming exported transition, suspicious self-transitions, and
outgoing transitions from terminal-looking state names such as `Published`,
`Done`, or `Archived`.

These are warning heuristics, not compile-time rejection rules. The lint claimed
authority surface is `StableGraphMetadata`; the actual observation point is the
serialized/exported metadata document. False positives are expected when runtime
policy, external guard conditions, domain vocabulary, or transition sites that
Statum rejected before metadata emission carry information the graph document
cannot observe. In particular, terminal-state detection is name-based today: a
state named `Published` with an intentional `unpublish` transition will be
reported until a richer explicit terminal marker exists. Validator-overlap
analysis is intentionally not claimed by this graph lint pass because validator
predicates are not represented in `StableGraphMetadata` v1.

A clean lint report still documents that boundary:

```text
Graph invariant lint report for showcases::axum_sqlite_review::DocumentMachine
authority: cfg_pruned_macro_input
false-positive boundary: lints inspect only StableGraphMetadata; runtime-only policy, external guard conditions, and transition sites rejected before metadata emission are outside this report.

No graph invariant warnings.
```

## Presentation Metadata

Structural introspection is separate from human-facing metadata.

If a consumer crate wants labels, descriptions, or phases for rendering, it can
add a typed `MachinePresentation` overlay keyed by the generated ids. That lets
the machine definition carry the supported structural metadata while the
consumer owns local explanation and presentation.

For lighter-weight cases, Statum can also emit a generated
`machine::PRESENTATION` constant from source-local attributes:

- `#[present(label = "...", description = "...")]` on the machine, state
  variants, and transition methods
- `#[presentation_types(machine = ..., state = ..., transition = ...)]` on the
  machine when you want typed `metadata = ...` payloads in the generated
  presentation surface

Typed presentation metadata follows the same observation point as the graph:
cfg-pruned attribute macro input and supported attribute shapes. If a category
declares `#[presentation_types(...)]`, each annotated item in that category
must supply `metadata = ...`; otherwise the macro rejects it instead of
guessing a default value.

## Example

Runnable example:
[statum-examples/src/toy_demos/16-machine-introspection.rs](../statum-examples/src/toy_demos/16-machine-introspection.rs)

For the authority boundary behind these metadata claims, see
[introspection-authority.md](introspection-authority.md). For a future
agent-facing MCP resource shape over the same metadata, see
[mcp-protocol-resource-design.md](mcp-protocol-resource-design.md). For comparing
committed graph snapshots during workflow migrations, see
[graph-diff-migrations.md](graph-diff-migrations.md).
