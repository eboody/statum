# `statum::testing` Helper API Design

This note sketches a future `statum::testing` surface for protocol-level tests.
It separates three jobs that should not share one API:

1. compile-time assertions that prove Rust code cannot express an illegal use;
2. runtime metadata assertions over generated machine graphs and rebuild reports;
3. generated fixtures that create typed machines, persisted rows, or legal walks
   for tests without hiding which invariant is being assumed.

The goal is not to turn Statum into a test framework. The goal is to give crates
using Statum small, explicit helpers for testing the protocol that Statum already
generates: legal transition sites, legal target states, typed rehydration, and
runtime joins between actual events and static graph metadata.

## Authority Surface

The testing helpers must state which observation point they use:

| Helper family | Observation point | May claim | Must not claim |
| --- | --- | --- | --- |
| Compile-time assertions | Rust compiler type checking for a concrete test crate or trybuild fixture | A snippet compiles or fails under the active feature set | The whole protocol graph is exhaustive |
| Runtime graph assertions | `MachineIntrospection::GRAPH` generated from cfg-pruned macro input | The emitted metadata contains or rejects a state/transition/target under the active build | Runtime side effects, storage correctness, or unsupported macro-generated transitions |
| Runtime transition-record assertions | `RecordedTransition` joined against `MachineIntrospection::GRAPH` | A recorded chosen branch is legal for its source transition | That the transition method body actually executed or persisted the event |
| Rehydration assertions | `Machine::rebuild`, `.build_report()`, `.explain()`, and batch report surfaces | A runtime input value matched, failed, or was ambiguous under generated validators | The backing store is complete, current, or the source of truth |
| Generated fixtures | Macro-generated state ids, builders, validators, and optional fixture traits | A test fixture was constructed through generated or explicitly unsafe/assume-state surfaces | That arbitrary caller data is valid without naming the proof or assumption |

Strict introspection remains the stronger graph mode. If a helper name or error
message says `exact`, it should either require `strict-introspection` or narrow
the claim to the current non-strict metadata behavior.

## Module Layout

Recommended public module:

```rust
pub mod testing {
    pub mod compile;
    pub mod graph;
    pub mod rehydrate;
    pub mod fixtures;
    pub mod walks;
    pub mod invariants;
}
```

Recommended Cargo exposure:

- keep lightweight runtime assertion helpers in `statum-core` behind a `testing`
  feature if they do not depend on proc-macro-only behavior;
- re-export them from `statum::testing`;
- keep generated fixture code in `statum-macros`, emitted only when the machine or
  validators opt in with an attribute such as `#[testing(fixtures)]` or when the
  crate enables a feature that is clearly documented as test-only;
- do not make production builds carry fixture constructors unless the caller asks
  for them.

## Compile-Time Assertions

Compile-time helpers are for tests that should fail before runtime. They should
lean on existing Rust testing patterns instead of inventing a new runner.

### Recommended API Shape

```rust
// In a normal test that should compile:
statum::testing::compile::assert_transition_callable::<
    DocumentMachine<Draft>,
    DocumentMachine<InReview>,
>("submit");
```

This helper can be a zero-sized, type-level assertion only if the generated
surface exposes enough trait evidence. If it cannot prove the method call itself,
it should not pretend to. The stronger and likely initial shape is documented
trybuild support:

```rust
#[test]
fn illegal_transition_fails_to_compile() {
    statum::testing::compile::trybuild()
        .compile_fail("tests/ui/document_approve_from_draft.rs")
        .pass("tests/ui/document_submit_from_draft.rs");
}
```

The helper would wrap `trybuild::TestCases`, set Statum-specific defaults, and
make feature-mode pairs easy to run:

```rust
statum::testing::compile::trybuild()
    .strict_introspection(false)
    .compile_fail("tests/ui/relaxed/*.rs");

statum::testing::compile::trybuild()
    .strict_introspection(true)
    .compile_fail("tests/ui/strict/*.rs");
```

### Assertions To Support

- legal method is callable from the source state;
- illegal method is not callable from another state;
- state-data fields are present only on the states that carry data;
- validator signatures produce typed rebuild output;
- strict-introspection rejects unsupported return shapes when the public claim is
  transition-target authority.

### Non-Goals

- Do not parse `.stderr` files for users. Trybuild already owns expected output.
- Do not promise exhaustive graph proof from one compile fixture.
- Do not require compile-time helpers for ordinary runtime graph assertions.

## Runtime Transition Assertions

Runtime graph assertions operate over `MachineIntrospection::GRAPH`. They should
be readable in ordinary tests and fail with machine, source state, method name,
and observed target lists.

### Recommended API Shape

```rust
use statum::testing::graph::{assert_transition, assert_no_transition, assert_targets};

let graph = <DocumentMachine<Draft> as statum::MachineIntrospection>::GRAPH;

assert_transition(graph)
    .from(document_machine::StateId::Draft)
    .method("submit")
    .to(document_machine::StateId::InReview);

assert_targets(graph)
    .from(document_machine::StateId::InReview)
    .method("decide")
    .exactly([
        document_machine::StateId::Published,
        document_machine::StateId::Rejected,
    ]);

assert_no_transition(graph)
    .from(document_machine::StateId::Draft)
    .method("approve");
```

The builder form gives room for clearer failure messages than a pile of
`assert_eq!` calls:

```text
expected transition DocumentMachine::Draft.approve to be absent
but GRAPH contains Draft --approve--> Published
```

### Helper Semantics

- `assert_transition(...).to(target)` passes when `transition_from_method(source,
  method)` exists and its legal target set contains `target`.
- `assert_targets(...).exactly(targets)` compares the transition site's emitted
  target set. Order should default to source order, with an opt-in
  `.as_set()` when order is not part of the user's assertion.
- `assert_no_transition(...)` checks the named source/method pair only. It should
  not claim there is no method with that name anywhere else in the machine.
- helpers should accept `&'static MachineGraph<S, T>` and work with generated
  `StateId` and `TransitionId` types where `S: Copy + Eq + Debug` and
  `T: Copy + Eq + Debug`.

## Runtime Metadata Assertions

Metadata assertions should also cover machine shape, state-data flags,
presentation overlays, and runtime transition records.

### State And Machine Shape

```rust
use statum::testing::graph::{assert_state, assert_machine};

assert_machine(graph)
    .rust_type_path("crate::workflow::DocumentMachine")
    .module_path(module_path!());

assert_state(graph)
    .id(document_machine::StateId::InReview)
    .rust_name("InReview")
    .has_data(true);
```

### Runtime Record Joins

```rust
use statum::testing::graph::assert_recorded_transition;

let event = <DocumentMachine<Draft> as statum::MachineTransitionRecorder>
    ::try_record_transition_to::<DocumentMachine<InReview>>(DocumentMachine::<Draft>::SUBMIT)
    .unwrap();

assert_recorded_transition(&event, graph)
    .from(document_machine::StateId::Draft)
    .chosen(document_machine::StateId::InReview)
    .method("submit");
```

This assertion should fail closed when the record's machine descriptor does not
match the graph, when the transition id is unknown, or when the chosen target is
not in the transition's legal target set.

### Presentation Metadata

Presentation assertions belong in a separate namespace so structural metadata and
human-facing labels do not blur:

```rust
statum::testing::graph::assert_presentation(presentation)
    .state(document_machine::StateId::InReview)
    .label("In review");
```

## Rehydration Assertions

Rehydration helpers are report-first. They should inspect `RebuildReport` rather
than rerunning validators unless the API name says it will call `.build_report()`
or `.explain()`.

### Single-Input Assertions

```rust
use statum::testing::rehydrate::{event_fixture, row_fixture, snapshot_fixture};

row_fixture(report)
    .rebuilds_as("InReview")
    .matched_by("is_in_review");

row_fixture(report)
    .fails()
    .candidate_states(["Draft", "InReview", "Published"])
    .unambiguous()
    .rejected_by("is_draft", "status_not_draft")
    .rejected_by("is_in_review", "missing_reviewer");

snapshot_fixture(snapshot_report)
    .rebuilds_as("Published")
    .matched_by("is_published");

// Event fixtures usually wrap an event-log projection report, not the raw event.
event_fixture(projected_event_report)
    .fails()
    .rejected_by("is_in_review", "missing_reviewer");
```

The API should prefer state and validator names already present in
`RebuildReport` rather than requiring generated state-id types. That keeps the
helper usable for invalid inputs where no typed machine exists.

### Ambiguity Assertions

```rust
row_fixture(report)
    .fails()
    .ambiguous_between(["Draft", "InReview"]);
```

Ambiguity assertions must require `.explain()` output or a report with
`RebuildAmbiguity::Ambiguous`. A normal `.build_report()` has
`RebuildAmbiguity::NotChecked`, so it cannot prove there was no second matching
validator.

### Batch Assertions

```rust
use statum::testing::rehydrate::assert_batch_reports;

assert_batch_reports(reports)
    .len(3)
    .slot(0).rebuilt_as("Draft")
    .slot(1).failed()
    .slot(2).rebuilt_as("Published")
    .preserves_input_order();
```

The batch helper should mirror the batch rehydration design: one slot per input,
stable order, partial failures preserved, and no storage-level correctness claim.

## Generated Fixtures

Fixtures should make legal tests convenient without erasing the difference
between constructed machines, rebuilt machines, and assumed machines.

### Fixture Families

1. Typed machine fixtures: build a concrete machine state through generated
   builders or explicitly generated test constructors.
2. Rehydration fixtures: produce persisted rows/documents plus expected rebuild
   reports.
3. Transition fixtures: produce starting machines and legal expected target ids.
4. Walk fixtures: generate finite legal state walks from graph metadata and a
   caller-provided transition executor.

### Opt-In Generated Trait

A machine-level opt-in can generate a test-only fixture trait:

```rust
#[machine]
#[testing(fixtures)]
struct DocumentMachine<DocumentState> {
    id: i64,
    title: String,
}
```

Generated shape:

```rust
pub trait DocumentMachineFixtures {
    fn draft_fixture() -> DocumentMachine<Draft>;
    fn in_review_fixture(review: ReviewAssignment) -> DocumentMachine<InReview>;
    fn published_fixture() -> DocumentMachine<Published>;
}
```

If a state carries data, the fixture constructor should either require that data
as an argument or use a user-supplied fixture provider. It should not silently
invent domain data unless the user supplied a default fixture policy.

### Rehydration Fixture Shape

```rust
pub struct RebuildFixture<Input, Machine> {
    pub input: Input,
    pub expected_state: &'static str,
    pub expected_validator: &'static str,
    pub expected_report: Option<statum::RebuildReport<Machine>>,
}
```

Generated validators can expose a builder for test rows only when the persisted
input type opts in. That keeps row construction near the type that knows its
storage shape.

### Assumed-State Fixtures

If the fixture bypasses validators, the name should say so:

```rust
let machine = statum::testing::fixtures::assume_state::<DocumentMachine<InReview>>(
    fields,
    ReviewAssignment { reviewer },
);
```

The docs should reserve this for unit tests where the input invariant is not the
subject under test. Integration tests should prefer generated builders or rebuild
fixtures.

## Legal Walk Generators

Legal walk generators use graph metadata to enumerate or sample sequences of
transition sites. They do not execute transition methods by themselves, because
method arguments and side effects are application-specific.

### Graph-Only Walks

```rust
use statum::testing::walks::{legal_walks_from, WalkOptions};

let walks = legal_walks_from(graph, document_machine::StateId::Draft)
    .max_depth(3)
    .terminal_states([document_machine::StateId::Published])
    .collect::<Vec<_>>();
```

A graph-only walk item can be:

```rust
pub struct WalkStep<S, T> {
    pub from: S,
    pub transition: T,
    pub method_name: &'static str,
    pub legal_targets: &'static [S],
}

pub struct LegalWalk<S, T> {
    pub start: S,
    pub steps: Vec<WalkStep<S, T>>,
    pub end: S,
}
```

For branching transitions, a walk should branch over each legal target. The
transition id stays the same; the chosen target differs.

### Executable Walks

Executable walks need caller-provided transition executors:

```rust
let outcome = statum::testing::walks::execute_walk(start_machine, walk)
    .on(DocumentMachine::<Draft>::SUBMIT, |machine, ctx| machine.submit(ctx.reviewer()))
    .on(DocumentMachine::<InReview>::APPROVE, |machine, _| machine.approve())
    .run(test_context)?;
```

This should be a later feature. It requires typed dispatch across heterogeneous
machine states and method argument providers. The initial API should ship
metadata walks first.

### Property-Based Generation

Optional `proptest` support should live behind a feature:

```rust
statum::testing::walks::proptest_legal_walks(graph)
    .start(document_machine::StateId::Draft)
    .max_depth(8)
```

The generator should shrink by removing suffixes first, then by choosing earlier
transition alternatives. It should reject impossible options, such as a start
state that is absent from the graph.

## Graph Invariant Checks

Graph invariant helpers are runtime metadata audits. They are useful in CI and in
examples because they test broad protocol shape without compiling dozens of UI
fixtures.

### Recommended Checks

```rust
use statum::testing::invariants::{assert_graph_invariants, InvariantPolicy};

assert_graph_invariants(graph, InvariantPolicy::default()
    .require_unique_state_names(true)
    .require_unique_transition_methods_per_source(true)
    .require_all_targets_declared(true)
    .require_no_duplicate_edges(true));
```

Default invariant checks should include:

- every transition source appears in `graph.states`;
- every transition target appears in `graph.states`;
- no duplicate transition id appears in the transition inventory;
- no duplicate `(from, method_name)` pair appears unless the generated API
  explicitly supports overloading that pair;
- each transition has at least one target;
- state `rust_name` values are unique within the machine.

Optional policy checks can include:

- every non-terminal state has at least one outgoing transition;
- every state is reachable from a declared start state;
- every declared terminal state has no outgoing transitions;
- no cycles, or cycles only through named transitions;
- branch transitions must name at least two targets;
- every state and transition has presentation metadata.

### Failure Output

Invariant failures should return structured data as well as panic-friendly text:

```rust
pub struct GraphInvariantFailure<S, T> {
    pub kind: GraphInvariantKind,
    pub state: Option<S>,
    pub transition: Option<T>,
    pub message: String,
}

pub fn check_graph_invariants<S, T>(
    graph: &'static MachineGraph<S, T>,
    policy: &InvariantPolicy<S>,
) -> Vec<GraphInvariantFailure<S, T>>;
```

`assert_graph_invariants` can panic if the vector is non-empty. The non-panicking
`check_` form is better for CLIs and CI reports.

## Implementation Order

1. Add runtime graph assertion helpers first. They are small wrappers around the
   existing `MachineGraph` methods and give immediate value for transition and
   path assertions.
2. Add invariant checks over `MachineGraph` without proc-macro changes.
3. Add rehydration report assertions over `RebuildReport` and batch report
   vectors.
4. Add metadata-only legal walk enumeration.
5. Add compile-time trybuild convenience helpers.
6. Add opt-in generated fixture traits once the assertion vocabulary is stable.
7. Explore executable walks and proptest support after the graph-only generator is
   proven useful.

## Test Plan For The Helpers

The helper implementation should include both ordinary Rust tests and trybuild
fixtures.

Runtime tests:

- assert a linear `Draft -> InReview -> Published` graph;
- assert a branching transition with two legal targets;
- assert absent source/method pairs;
- assert state-data flags;
- assert a valid `RecordedTransition` join and a mismatched-machine failure;
- assert failed, successful, ambiguous, and not-checked `RebuildReport` cases;
- assert batch report order with mixed success and failure slots;
- assert invariant failures for unknown targets, duplicate ids, duplicate edges,
  unreachable states, and terminal states with outgoing transitions.

Compile-time tests:

- legal transition method compiles from its source state;
- illegal transition method fails from the wrong source state;
- state-data access fails on states without data;
- strict-introspection rejects unsupported shapes used by graph assertions;
- fixture opt-in code is absent unless the attribute or feature is enabled.

Adversarial semantic tests:

- `#[cfg]` changes the emitted graph under different feature sets;
- macro-generated transition methods outside Statum's observation point are not
  claimed by graph assertions;
- `include!` or alias-heavy return shapes are rejected or explicitly annotated
  before helpers claim exact targets;
- duplicate method names on different source states remain distinguishable by
  `(source, method_name)`.

## Acceptance Checklist

- Compile-time assertion APIs are documented separately from runtime assertion
  APIs.
- Runtime metadata assertions are documented separately from generated fixtures.
- Rehydration assertions inspect `RebuildReport` evidence and do not make storage
  correctness claims.
- Path and walk helpers are graph metadata tools unless a later executable-walk
  API explicitly asks the caller for transition executors.
- Fixture APIs name whether they build, rebuild, or assume typed state.
- Any helper that says `exact` names strict introspection or states its weaker
  observation point.
