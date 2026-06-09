# Property-Based Legal Workflow Generation Spike

## Verdict: PARTIAL

Metadata-only legal walk generation is feasible now, using the emitted
`MachineGraph<S, T>` as the observation point. The spike adds a small
`statum_core::testing::walks` prototype that enumerates finite legal walks and
branches over each declared target for branching transitions. Full property-test
integration should wait for a second step because the public API needs a feature
and dependency strategy that does not force `proptest` onto normal users.

## Question

Can Statum generate legal random walks from machine metadata so users can test
workflow-level properties without hand-writing every transition path?

## Authority Surface

Claimed surface: legal graph-metadata walks for the active build.

Actual observation point: `MachineGraph<S, T>` emitted by Statum macros after the
active `cfg` configuration. The prototype does not inspect transition method
bodies, execute side effects, synthesize method arguments, query persisted state,
or prove that macro-generated code outside Statum's metadata surface exists.

Unsupported cases must stay explicit:

- executable walks need caller-provided transition executors and argument
  providers;
- storage-backed workflow legality needs runtime event/report assertions, not a
  graph-only generator;
- strict target authority still follows the existing `strict-introspection`
  boundaries for transition metadata;
- unbounded cyclic graphs are not enumerated without a caller-provided depth.

## Prototype API

The spike implements a metadata-only iterator shape in
`statum-core/src/testing/walks.rs`:

```rust
use statum::testing::walks::legal_walks_from;

let walks = legal_walks_from(graph, document_machine::StateId::Draft)
    .max_depth(4)
    .terminal_states([document_machine::StateId::Published])
    .include_empty(false)
    .into_iter()
    .collect::<Vec<_>>();
```

Each `LegalWalk` contains a start state, an end state, and ordered `WalkStep`s.
Each `WalkStep` records:

- `from`: source state for the step;
- `transition`: transition-site id;
- `method_name`: Rust method name from metadata;
- `legal_targets`: the complete target slice for that transition site;
- `chosen_target`: the target selected for this branch.

Branching transitions produce one walk branch per declared legal target. The
transition id stays stable; only `chosen_target` changes.

## Feasibility Findings

What worked:

- Existing `MachineGraph::state`, `transitions_from`, and transition descriptors
  are enough to enumerate finite legal walks without macro changes.
- The core generator does not require `proptest`; deterministic enumeration is a
  useful base for assertions, examples, and later strategy construction.
- A depth limit makes cyclic graphs deterministic and prevents runaway
  enumeration.
- Terminal states are a caller policy, not a graph fact. Keeping them as builder
  input avoids pretending Statum knows domain-specific workflow completion.

What did not fit the first slice:

- Executable walks need typed dispatch across heterogeneous machine states and
  method-specific argument providers. That should remain a later API.
- Random generation and shrinking are better as an adapter over the deterministic
  enumerator, not as the core representation.
- `proptest` cannot be a normal dev-dependency if downstream users need the
  public helper. It must be an optional dependency behind a feature.

## Recommended Property-Test API

Recommended feature wiring for a later implementation:

```toml
[dependencies]
proptest = { version = "1", optional = true, default-features = false, features = ["std"] }

[features]
default = []
testing-proptest = ["testing", "dep:proptest"]
```

Recommended API shape:

```rust
#[cfg(feature = "testing-proptest")]
pub fn proptest_legal_walks<S, T>(
    graph: &'static MachineGraph<S, T>,
) -> LegalWalkStrategy<S, T>
where
    S: Copy + Eq + Debug + 'static,
    T: Copy + Eq + Debug + 'static;
```

Builder options should mirror the deterministic generator:

```rust
proptest_legal_walks(graph)
    .start(document_machine::StateId::Draft)
    .max_depth(8)
    .terminal_states([document_machine::StateId::Published]);
```

Shrinking policy:

1. shrink by shortening suffixes first;
2. shrink branch choices toward earlier graph-order transition alternatives;
3. shrink `max_depth`-bounded walks without generating states absent from the
   graph.

Invalid options should fail before generating cases: absent start state,
`max_depth` too high for a caller-defined budget, or terminal states not present
in the graph when the caller asks for strict option validation.

## Compile And Dependency Tradeoffs

The deterministic enumerator adds no new third-party dependency. It uses `Vec`,
so it matches the current `std`-using `statum-core` testing helper direction.

`proptest` should stay optional because:

- it is materially larger than the core graph assertion helpers;
- downstream public helpers cannot be exposed from dev-dependencies;
- users who only want metadata assertions should not pay property-testing compile
  cost;
- Cargo feature unification would otherwise make `proptest` contagious for crates
  that depend on `statum` in test builds.

The public facade should re-export the feature from `statum` to `statum-core` so
users can enable one feature on the main crate:

```toml
statum = { version = "...", features = ["testing-proptest"] }
```

## Test Coverage Added

The prototype includes unit coverage for:

- branching walk prefix enumeration;
- terminal states stopping expansion before outgoing transitions;
- depth-bounded cycles;
- absent start states yielding no walks.

Adversarial cases still needed for a production proptest adapter:

- `#[cfg]` changes to emitted graph metadata across feature sets;
- duplicate method names on different sources staying distinguishable by source;
- branch order remaining stable for shrinking;
- strict-introspection rejection cases when target metadata would otherwise be
  weaker than the helper claim.

## Recommendation

Ship metadata-only legal walk enumeration before random property-test support.
It is dependency-free, improves protocol assertions immediately, and gives the
`proptest` adapter a small, testable source of truth. Add `testing-proptest` only
when the deterministic walk API has settled and can be re-exported through the
public `statum` crate without widening normal compile cost.
