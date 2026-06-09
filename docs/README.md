# Statum Documentation Map

Read these docs by the job you are doing, not by the macro you are curious
about. Statum's macros are implementation hooks for a workflow protocol: named
phases, phase-specific data, legal transitions, typed rehydration, batch rebuilds,
event-log projection, diagnostics, and generated metadata.

If you are new, start with [Start here](start-here.md). This page is the longer
map for choosing the next document.

## Start A Workflow Protocol

Use these when you need to decide whether Statum fits and build the smallest
useful machine.

- [Start here](start-here.md): the short evaluation path.
- [When not to use Statum](why-not-just-an-enum.md): compare plain enums,
  runtime validation, builder crates, runtime state machines, and Statum
  typestate.
- [Tutorial: grow a review workflow one feature at a time](tutorial-review-workflow.md):
  begin with a tiny document workflow, then add shared context, transitions,
  phase data, validators, boundary matching, and graph output only when the
  workflow needs them.
- [Patterns and guidance](patterns.md): recurring design patterns once the core
  model fits.

## Carry Phase Data Safely

Use these when different phases need different fields or methods.

- The review tutorial's phase-data step:
  [Make the workflow real](tutorial-review-workflow.md#4-make-the-workflow-real-add-review-and-state-data).
- [Generated builder reference](generated-builder-reference.md): required shared
  fields, state payload fields, visibility, generics, rebuild builders, and
  known generated-surface limits.
- [Typestate builder design playbook](typestate-builder-design-playbook.md):
  decide whether you need a durable workflow machine or a smaller staged builder
  surface.
- [Builder UX positioning](builder-ux-positioning.md): compare Statum's generated
  builders with ordinary builder crates.

## Encode Legal Transitions

Use these when your main risk is a value moving to the wrong next phase.

- The review tutorial's first transition step:
  [Add the first legal transition](tutorial-review-workflow.md#3-add-the-first-legal-transition).
- [Patterns and guidance](patterns.md): modeling linear, branching, nested, and
  side-effecting workflows.
- [When not to use Statum](why-not-just-an-enum.md): sanity-check whether the
  transition graph is stable enough for typestate.

## Rehydrate Persisted State

Use these when rows, JSON, events, or external records need to become typed
machines before app code continues.

- [Typed rehydration and validators](persistence-and-validators.md): single-item
  rebuilds, report/explain mode, async validators, batches, integration
  boundaries, and failure model.
- [Rehydration vocabulary](rehydration-vocabulary.md): project, rebuild,
  rehydrate, recover, explain, and escape-hatch terminology.
- [Escape hatches](escape-hatches.md): unchecked or assume-state surfaces and
  introspection overrides to audit carefully.
- [Migration guide](migration.md): move older Statum code to current validator,
  transition, builder, and helper names.

## Process Batches And Event Logs

Use these when the system is not just rebuilding one value at a time.

- [Event-log rebuild case study](case-study-event-log-rebuild.md): append-only
  events, projection into snapshots, typed rebuild, and legal next events.
- [Batch rehydration helper design](batch-rehydration-design.md): machine-scoped
  batch helper vocabulary and design tradeoffs.
- Showcase code:
  [sqlite_event_log_rebuild.rs](../statum-examples/src/showcases/sqlite_event_log_rebuild.rs)
  and
  [serde_json_snapshot.rs](../statum-examples/src/showcases/serde_json_snapshot.rs).

## Explain, Inspect, And Generate Metadata

Use these when tooling, docs, tests, or agents need a graph view of the workflow.

- [Machine introspection](introspection.md): generated graph metadata, transition
  identity, runtime joins, and presentation metadata.
- [Introspection authority boundaries](introspection-authority.md): what graph
  metadata observes across raw source, macro input, cfg pruning, expansion,
  type checking, runtime registry values, and persisted state.
- [Graph diffing for workflow migrations](graph-diff-migrations.md): compare
  stable graph snapshots and surface migration concerns in CI and PR comments.
- [Protocol docs generation](protocol-doc-generation.md): render Mermaid,
  transition tables, and narrative summaries from one metadata value and keep
  generated artifacts current.
- [`statum::testing` helper API design](testing-helper-api-design.md): future
  helpers for compile-time assertions, runtime graph/report assertions, generated
  fixtures, legal walks, and graph invariants.
- [Property-based legal workflow generation spike](legal-walk-generation-spike.md):
  feasibility, API shape, and dependency tradeoffs for metadata-only walks and a
  future optional `proptest` adapter.
- [MCP/protocol resource design](mcp-protocol-resource-design.md): future-facing
  resource shape for exposing protocol metadata to tools.
- [Agent maintainer checklist](agents/maintainer-checklist.md): review protocol
  docs and agent-facing instructions without widening authority claims.
- [Statum for coding agents](agents/README.md): templates and prompts for teams
  that want agents to spot workflow-protocol opportunities.

Introspection authority is scoped. Strict mode derives transition targets from
cfg-pruned macro input: readable transition signatures or explicit
`#[introspect(return = ...)]` annotations. Unsupported shapes are rejected
rather than approximated; the authority page names the weaker surfaces Statum
does not claim.

## Diagnose, Migrate, And Measure

Use these when you are maintaining Statum itself or upgrading a codebase.

- [Diagnostics guide](diagnostics/README.md): broken fixtures, expected compiler
  output, corrected shapes, and explanations for the known diagnostic surface.
- [Diagnostics quality audit](diagnostics-quality-audit.md): diagnostic standards,
  accepted compiler fallbacks, and polish targets.
- [Compile-time benchmark reporting](compile-time-benchmark-reporting.md): how to
  run and report compile-time benchmarks.
- [Compile-time benchmark baseline](compile-time-benchmark-baseline.md): current
  benchmark baseline and interpretation.
- [World-class quality roadmap](world-class-roadmap.md): quality bar across
  diagnostics, builder UX, compile-time reporting, examples, and docs.

## Run Service-Shaped Examples

Use these after reading the tutorial when you want application-shaped code.

```bash
cargo run -p statum-examples --bin axum-sqlite-review
cargo run -p statum-examples --bin clap-sqlite-deploy-pipeline
cargo run -p statum-examples --bin sqlite-event-log-rebuild
cargo run -p statum-examples --bin tokio-sqlite-job-runner
cargo run -p statum-examples --bin tokio-websocket-session
```

There are no Cargo `[[example]]` targets in this repository; the showcase entry
points are binaries under `statum-examples/src/bin/`.
