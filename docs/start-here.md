# Start Here

If you are evaluating Statum from the outside, do not read the repo front to
back. Use this short path instead.

## 1. Read The README Quick Start

Start with the root [README](../README.md):

- the install snippet
- the 60-second example
- the mental model for `#[state]`, `#[machine]`, `#[transition]`, and
  `#[validators]`

That is enough to decide whether the API shape is interesting.

## 2. Read The Flagship Case Study

Then read [case-study-event-log-rebuild.md](case-study-event-log-rebuild.md).

That is the strongest Statum story in this repo:

- append-only events
- projection into row-like snapshots
- typed rehydration back into legal machine states
- no ad hoc status branching after rebuild

If that problem shape matters to you, Statum is probably worth a deeper look.

## 3. Open One App-Shaped Example

Use [axum-sqlite-review](../statum-examples/src/showcases/axum_sqlite_review.rs)
if you want the most approachable service example.

It shows:

- a small HTTP workflow
- SQLite-backed typed rehydration on each request
- transitions that stay explicit at the handler boundary

## 4. Go Deeper Only Where Needed

Use the focused docs rather than reading everything:

- [Typed rehydration and validators](persistence-and-validators.md)
- [Patterns and guidance](patterns.md)
- [Migration guide](migration.md) if you are upgrading an older Statum codebase
- [Typestate builder design playbook](typestate-builder-design-playbook.md) if
  you are deciding whether a workflow is a good fit

## 5. Use The Agent Kit Only If It Matches Your Workflow

If you work with coding agents and want them to spot Statum opportunities in
your own repo, start with [agents/README.md](agents/README.md).

That is optional. It is not the main evaluation path for the crate itself.
