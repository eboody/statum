# statum-graph

`statum-graph` exports static machine topology directly from
`statum::MachineIntrospection::GRAPH`.

It is authoritative only for machine-local structure:

- machine identity
- states
- transition sites
- exact legal targets
- graph roots derivable from the static graph itself

For linked-build codebase export, `statum-graph` can also combine every linked
compiled machine family, legacy direct payload links, declared validator-entry
surfaces, direct-construction availability per state, and exact relation
records inferred from supported type syntax, `#[via(...)]` declarations, and
nominal `#[machine_ref(...)]` declarations. That codebase view is still static
only. It does not model runtime-selected branches or orchestration order
across machines. Validator node labels come from the impl self type as written
in source and are display-only, not canonical Rust type identity.
Method-level `#[cfg]` and `#[cfg_attr]` on validator methods are rejected at
the macro layer. `include!()`-generated validator impls are also rejected. In
v1, exact direct-type relations recurse only through canonical absolute carrier
paths such as `::core::option::Option<...>` and
`::core::result::Result<..., E>`, and direct machine targets must use explicit
`crate::`, `self::`, `super::`, or absolute paths instead of imported aliases
or bare names.

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

## Canonical Export Model

`MachineDoc` is the validated typed graph surface. `ExportDoc` is the stable
renderer-facing model built from that graph:

```rust
# use statum::{machine, state, transition};
# use statum_graph::MachineDoc;
# #[state]
# enum FlowState {
#     Draft,
#     Review,
#     Accepted,
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
# }
let doc = MachineDoc::from_machine::<Flow<Draft>>();
let export = doc.export();

assert_eq!(export.states()[0].index, 0);
assert_eq!(export.transitions()[0].method_name, "submit");
```

If you have matching `MachinePresentation` metadata, join it onto the export
surface before rendering:

```rust
# use statum::{machine, state, transition};
# use statum_graph::{render, MachineDoc};
# #[state]
# enum PresentedState {
#     #[present(label = "Queued")]
#     Queued,
#     Done,
# }
# #[machine]
# #[present(label = "Presented Flow")]
# struct PresentedFlow<PresentedState> {}
# #[transition]
# impl PresentedFlow<Queued> {
#     #[present(label = "Finish")]
#     fn finish(self) -> PresentedFlow<Done> {
#         self.transition()
#     }
# }
let doc = MachineDoc::from_machine::<PresentedFlow<Queued>>();
let export = doc.export_with_presentation(&presented_flow::PRESENTATION)?;

assert_eq!(export.machine().label, Some("Presented Flow"));
assert_eq!(render::mermaid(&export).contains("Finish"), true);
# Ok::<(), statum_graph::ExportDocError>(())
```

Presentation metadata can change labels and descriptions, but the structure
still comes from `MachineIntrospection::GRAPH`.

## Other Renderers

The same `ExportDoc` drives every built-in renderer:

```rust
# use statum::{machine, state, transition};
# use statum_graph::{render, MachineDoc};
# #[state]
# enum FlowState {
#     Draft,
#     Done,
# }
# #[machine]
# struct Flow<FlowState> {}
# #[transition]
# impl Flow<Draft> {
#     fn finish(self) -> Flow<Done> {
#         self.transition()
#     }
# }
let doc = MachineDoc::from_machine::<Flow<Draft>>();
let export = doc.export();

let mermaid = render::mermaid(&export);
let dot = render::dot(&export);
let plantuml = render::plantuml(&export);
let json = render::json(&export);

assert!(mermaid.contains("graph TD"));
assert!(dot.contains("digraph"));
assert!(plantuml.contains("@startuml"));
assert!(json.contains("\"transitions\""));
```

The JSON renderer is stable and pretty-printed. It exports machine identity,
states, transition sites, exact legal targets, roots, and optional labels and
descriptions. It does not serialize arbitrary typed `metadata` payloads from
`MachinePresentation`; those stay application-owned unless you define a
separate serialization contract.

## Codebase Export

If you want one combined graph for the linked build instead of one machine at a
time, use `CodebaseDoc`:

```rust
# use statum::{machine, state, transition};
# use statum_graph::CodebaseDoc;
# mod task {
#     use statum::{machine, state, transition};
#     #[state]
#     pub enum State {
#         Idle,
#         Running,
#     }
#     #[machine]
#     pub struct Machine<State> {}
#     #[transition]
#     impl Machine<Idle> {
#         fn start(self) -> Machine<Running> {
#             self.transition()
#         }
#     }
# }
# mod workflow {
#     use super::task;
#     use statum::{machine, state, transition};
#     #[state]
#     pub enum State {
#         Draft,
#         InProgress(super::task::Machine<super::task::Running>),
#     }
#     #[machine]
#     pub struct Machine<State> {}
#     #[transition]
#     impl Machine<Draft> {
#         fn start(
#             self,
#             task: super::task::Machine<super::task::Running>,
#         ) -> Machine<InProgress> {
#             self.transition_with(task)
#         }
#     }
# }
let codebase = CodebaseDoc::linked()?;

assert!(codebase.machines().len() >= 2);
assert!(!codebase.links().is_empty());
assert!(!codebase.relations().is_empty());
# Ok::<(), statum_graph::CodebaseDocError>(())
```

Render or write the combined document through `statum_graph::codebase::render`:

```rust
# use statum_graph::{CodebaseDoc, codebase::render};
# let doc = CodebaseDoc::linked()?;
let mermaid = render::mermaid(&doc);
let paths = render::write_all_to_dir(&doc, "out", "codebase")?;

assert!(mermaid.contains("graph TD"));
assert_eq!(paths.len(), 4);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The codebase view is based on the linked compiled build, not a source scan.
Legacy `links()` come only from direct machine-like payload types written in
state data, including named fields. The richer exact `relations()` surface also
covers machine fields, transition parameters, `#[via(...)]` declarations, and
nominal opaque reference types declared once with `#[machine_ref(...)]`. In
v1, `#[machine_ref(...)]` supports nominal structs and tuple structs only;
plain type aliases are rejected. Exact direct-type relations recurse only
through canonical absolute carrier paths such as `::core::option::Option<...>`
and `::core::result::Result<..., E>`, and direct machine targets must use
explicit `crate::`, `self::`, `super::`, or absolute paths. Validator-entry
nodes come only from compiled `#[validators]` impls and represent declared
rebuild surfaces such as `DbRow::into_machine()`, not runtime match outcomes.
All exact surfaces fail closed on malformed or ambiguous linked metadata.
Transition-body orchestration, runtime composition, primitive ids with no
typed wrapper, and terminal-state semantics are intentionally out of scope.

`#[via(...)]` is the exact relation surface for “this parent transition depends
on this exact child transition route.” For example, if one transition takes
`crate::PaymentMachine<crate::Captured>` and also declares
`#[via(self::payment_machine::via::Capture)]`,
`CodebaseDoc` can say both:

- the parent transition depends on the child being in `Captured`
- the parent transition can depend on `PaymentMachine<Authorized>::capture`

This improves exact relation detail without inferring a protocol-stage graph.
For a runnable example that also asserts the linked relation basis, see
[`statum-examples/src/toy_demos/17-attested-composition.rs`](../statum-examples/src/toy_demos/17-attested-composition.rs).
Graph backends mark directly constructible states with a ` [build]` suffix and
derive cross-machine summary edges from exact `relations()`. Downstream
consumers can use `machine_relation_groups()`, inbound and outbound relation
lookup helpers, and `relation_detail()` to drive exact navigation without
re-deriving relation semantics. The codebase surface also carries source
rustdoc separately as `docs` on machines, states, transitions, and validator
entries. Use `#[present(description = ...)]` for concise UI copy and outer
rustdoc comments (`///`) for fuller inspector and `codebase.json` detail.

If you do not want to hand-write a runner crate, install
`cargo-statum-graph` and point it at an existing library package:

```text
cargo statum-graph codebase \
  /path/to/workspace
```

That command synthesizes the runner internally and writes the same four-file
bundle into the workspace root without requiring exporter code in the target
crate. Use `--out-dir` to override the destination or `--package` to narrow a
multi-package workspace to one library crate.

## Writing Files

You can also write one file directly:

```rust
# use statum::{machine, state, transition};
# use statum_graph::{render, MachineDoc};
# #[state]
# enum FlowState {
#     Draft,
#     Done,
# }
# #[machine]
# struct Flow<FlowState> {}
# #[transition]
# impl Flow<Draft> {
#     fn finish(self) -> Flow<Done> {
#         self.transition()
#     }
# }
let doc = MachineDoc::from_machine::<Flow<Draft>>();
render::Format::Mermaid.write_to(&doc, "out/flow.mmd")?;
# Ok::<(), std::io::Error>(())
```

Or write the whole built-in bundle with standard extensions:

```rust
# use statum::{machine, state, transition};
# use statum_graph::{render, MachineDoc};
# #[state]
# enum FlowState {
#     Draft,
#     Done,
# }
# #[machine]
# struct Flow<FlowState> {}
# #[transition]
# impl Flow<Draft> {
#     fn finish(self) -> Flow<Done> {
#         self.transition()
#     }
# }
let doc = MachineDoc::from_machine::<Flow<Draft>>();
let paths = render::write_all_to_dir(&doc, "out", "flow")?;

assert_eq!(paths.len(), 4);
# Ok::<(), std::io::Error>(())
```

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

If you join presentation metadata onto a validated machine graph, malformed
presentation overlays fail closed with `ExportDocError` instead of picking a
best-effort winner.

## Scope

`statum-graph` exports static machine-local topology. It does not tell you
which branch ran in one execution, how multiple machines were orchestrated at
runtime, or how machine data changed over time. For those use cases, pair the
static graph with explicit runtime events or snapshots from the application.
