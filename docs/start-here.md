# Start Here

If you are evaluating Statum from the outside, don't read the repo front to
back. Use this short path instead.

Keep one question in mind while reading: does this value's phase need to change
what methods are legally available on it?

Statum is for cases where invalid, undesirable, or not-yet-validated states
should not survive as ordinary values in your core API.

That often looks like a durable workflow. It can also be a smaller validated,
resolved, or build-ready surface where later phases should expose different
operations than earlier ones.

## 1. Read The README Quick Start

Start with the root [README](../README.md):

- the install snippet
- the 60-second example
- the mental model for `#[state]`, `#[machine]`, `#[transition]`, and
  `#[validators]`

That is enough to decide whether Statum fits your correctness problem.

## 2. Read The Guided Tutorial

Then read [tutorial-review-workflow.md](tutorial-review-workflow.md).

This is the canonical document-approval workflow for the repo. It is the
progressive path for understanding how the pieces fit together in an app-shaped
workflow. It starts with the smallest working machine, then adds the next
feature only when the workflow needs it:

- `#[state]`
- `#[machine]`
- `#[transition]`
- `#[validators]`
- matching reconstructed machines at the HTTP boundary
- generated graph edges for tooling and docs

## 3. Open The Runnable Service Example

Open [axum-sqlite-review](../statum-examples/src/showcases/axum_sqlite_review.rs)
next. It is the service-shaped version of the tutorial.

It shows:

- state-specific review assignment data
- legal `submit` and `approve` transitions
- SQLite-backed typed rehydration before each transition
- graph output from macro-generated introspection metadata

## 4. Read The Event-Log Companion

Then read [case-study-event-log-rebuild.md](case-study-event-log-rebuild.md).

That is the persistence-focused companion to the document-approval path:

- append-only events
- projection into row-like snapshots
- typed rehydration back into legal machine states
- no ad hoc status branching after rebuild

It reinforces the same core claim: raw persisted facts stay raw until they can
be proven to represent one legal state.

If that problem shape matters to you, Statum is probably worth a deeper look.

## 5. Go Deeper By Job

Use the [documentation map](README.md) rather than reading features in macro
order. Pick the job you are doing:

- Start a workflow protocol: [When not to use Statum](why-not-just-an-enum.md),
  [Patterns and guidance](patterns.md), and the guided tutorial.
- Carry phase data safely: the tutorial's state-data step,
  [Generated builder reference](generated-builder-reference.md),
  [Typestate builder design playbook](typestate-builder-design-playbook.md), and
  [Builder UX positioning](builder-ux-positioning.md).
- Encode legal transitions: the tutorial's transition steps plus patterns for
  branching, nested, and side-effecting workflows.
- Rehydrate persisted state: [Typed rehydration and validators](persistence-and-validators.md),
  [Rehydration vocabulary](rehydration-vocabulary.md),
  [Escape hatches](escape-hatches.md), and [Migration guide](migration.md).
- Process batches and event logs: [Event-log rebuild case study](case-study-event-log-rebuild.md)
  and [Batch rehydration helper design](batch-rehydration-design.md).
- Explain or generate metadata: [Machine introspection](introspection.md),
  [MCP/protocol resource design](mcp-protocol-resource-design.md), and the
  [agent docs](agents/README.md).
- Diagnose, migrate, or measure Statum itself: [Diagnostics guide](diagnostics/README.md),
  [Diagnostics quality audit](diagnostics-quality-audit.md),
  [Compile-time benchmark reporting](compile-time-benchmark-reporting.md),
  [Compile-time benchmark baseline](compile-time-benchmark-baseline.md), and
  [World-class quality roadmap](world-class-roadmap.md).

## 6. Use The Agent Kit Only If It Matches Your Workflow

If you work with coding agents and want them to spot Statum opportunities in
your own repo, start with [agents/README.md](agents/README.md).

That is optional. It is not the main evaluation path for the crate itself.

## Toolchain And Feature Flags

The repository's local toolchain file tracks stable Rust, but the published
minimum is the workspace `rust-version = "1.93"`. CI checks that minimum with
Rust `1.93.1`, while the normal stable job runs format, link checks, clippy,
tests, workspace hygiene, and docs.

There are no default crate features. Enable `statum/strict-introspection` when
generated graph metadata must fail closed instead of accepting ergonomic source
aliases. In that mode, transition targets come from directly readable
`#[transition]` signatures or explicit `#[introspect(return = ...)]`
annotations; unsupported shapes are rejected.

The workspace intentionally mixes editions: `statum` and `statum-core` are Rust
2021 crates, while `statum-macros` and `statum-examples` are Rust 2024 crates.
