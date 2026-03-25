# statum-graph

`statum-graph` exports static machine topology directly from
`statum::MachineIntrospection::GRAPH`.

It is authoritative only for machine-local structure:

- machine identity
- states
- transition sites
- exact legal targets
- roots derivable from the static graph itself

It does not model runtime-selected branches, orchestration across multiple
machines, or consumer-owned explanation metadata.

## Install

```toml
[dependencies]
statum = "0.6.9"
statum-graph = "0.6.9"
```

## Example

```rust
use statum::{machine, state, transition};
use statum_graph::{render, MachineDoc};

#[state]
enum FlowState {
    Draft,
    Review,
    Accepted,
    Rejected,
}

#[machine]
struct Flow<FlowState> {}

#[transition]
impl Flow<Draft> {
    fn submit(self) -> Flow<Review> {
        self.transition()
    }
}

#[transition]
impl Flow<Review> {
    fn decide(
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

let doc = MachineDoc::from_machine::<Flow<Draft>>();
let mermaid = render::mermaid(&doc);

assert!(mermaid.contains("s1 -->|decide| s2"));
assert!(mermaid.contains("s1 -->|decide| s3"));
```

## Mermaid Output

The renderer returns ordinary Mermaid flowchart text:

```text
graph TD
    s0["Draft"]
    s1["Review"]
    s2["Accepted"]
    s3["Rejected"]

    s0 -->|submit| s1
    s1 -->|accept| s2
    s1 -->|decide| s2
    s1 -->|decide| s3
    s1 -->|reject| s3
```

The output is deterministic for one validated `MachineDoc`, so it works well
for snapshot tests, generated docs, and CLI output.

## Traversing A Graph

`MachineDoc` gives you the machine descriptor, the state list, the transition
sites, and the root states:

```rust
# use statum::{machine, state, transition};
# use statum_graph::MachineDoc;
# #[state]
# enum FlowState {
#     Draft,
#     Review,
#     Accepted,
#     Rejected,
# }
# #[machine]
# struct Flow<FlowState> {}
# #[transition]
# impl Flow<Draft> {
#     fn submit(self) -> Flow<Review> {
#         self.transition()
#     }
# }
# #[transition]
# impl Flow<Review> {
#     fn accept(self) -> Flow<Accepted> {
#         self.transition()
#     }
#     fn reject(self) -> Flow<Rejected> {
#         self.transition()
#     }
# }
let doc = MachineDoc::from_machine::<Flow<Draft>>();

assert!(doc.machine().rust_type_path.ends_with("Flow"));
assert_eq!(
    doc.roots()
        .map(|state| state.descriptor.rust_name)
        .collect::<Vec<_>>(),
    vec!["Draft"]
);
assert_eq!(
    doc.states()
        .iter()
        .map(|state| state.descriptor.rust_name)
        .collect::<Vec<_>>(),
    vec!["Draft", "Review", "Accepted", "Rejected"]
);
assert_eq!(
    doc.edges()
        .iter()
        .map(|edge| edge.descriptor.method_name)
        .collect::<Vec<_>>(),
    vec!["submit", "accept", "reject"]
);
```

Use `doc.state(id)` when you need to map a transition target id back to the
exported state descriptor.

## Choosing An Entry Point

Use `MachineDoc::from_machine::<M>()` when the graph comes from a real Statum
machine type. That is the normal entry point for application code, test
assertions, and generated documentation.

Use `MachineDoc::try_from_graph(...)` when you already have a
`statum::MachineGraph` and want `statum-graph` to validate it before rendering
or traversal. This is mainly for external graph producers, tests, and tooling
adapters.

`try_from_graph(...)` rejects malformed graphs instead of guessing:

- empty state lists
- duplicate state ids
- duplicate transition ids
- duplicate transition sites for one `(source state, method name)` pair
- missing source states
- missing target states
- empty target sets
- duplicate target states within one transition

The error surface is `MachineDocError`.

## Scope

`statum-graph` exports static machine-local topology. It does not tell you
which branch ran in one execution, how multiple machines were orchestrated at
runtime, or how machine data changed over time. For those use cases, pair the
static graph with explicit runtime events or snapshots from the application.
