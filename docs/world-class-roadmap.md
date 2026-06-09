# World-Class Quality Roadmap

This document turns Statum's current strengths into an explicit quality bar.
It is not a feature wishlist. It is the acceptance standard for making Statum
feel like a polished Rust typestate workflow framework instead of a clever macro
experiment.

## Positioning

Statum should be presented as a typestate workflow framework, not as a general
builder crate.

The shortest accurate description is:

> Statum makes stable workflow phases, legal transitions, and persisted-state
> rehydration explicit in Rust's type system.

That keeps the pitch centered on the hard problem Statum owns:

- phase-specific method surfaces,
- state-specific data,
- legal transition edges,
- rebuilding typed machines from dynamic rows or events,
- optional introspection when tooling needs the graph.

Dedicated builder crates are better for ordinary construction ergonomics:
defaults, optional fields, setter customization, and broad derive coverage.
Statum's generated builders exist to construct typed machines and rebuild typed
machines; they are supporting API, not the category.

## Product Quality Bar

### 1. Public story

A reader should understand these boundaries quickly:

- Use Statum when a value's phase should change what methods are legal.
- Use validators when persisted or external facts must become typed machines.
- Use introspection when tooling needs the legal graph.
- Use an ordinary builder crate when the problem is only data assembly.

Acceptance checks:

- README names typed rehydration as a first-class differentiator.
- Start-here path points evaluators to the document-approval flagship and the
  event-log persistence companion.
- Builder docs say what Statum intentionally does not compete with.

### 2. Diagnostics

Macro errors should feel designed, not leaked from implementation internals.
Every first-party diagnostic should answer:

1. What did Statum reject?
2. Which machine/state/method/field caused it?
3. What shape was found?
4. What shape was expected?
5. How should the user fix it?

Acceptance checks:

- New first-party diagnostics use `Error`, `Found`, `Expected`, and `Fix` when
  those fields make sense.
- Diagnostics name the relevant state enum, machine, transition method, or
  validator method.
- Compile-fail fixtures are grouped by subsystem and reviewed before release.
- Rust compiler fallback errors are accepted only when the generated surface is
  intentionally missing, such as legacy API compatibility tests.

### 3. Builder UX

Generated builders should be evaluated as Statum-specific builders, not as a
replacement for dedicated builder crates.

Acceptance checks:

- Initial machine builders have predictable method names and collision errors.
- Rebuild builders have type-first entry points: `Machine::rebuild(row)` and
  `Machine::rebuild_many(rows)`.
- Batch rebuilds support both shared machine context and per-item context.
- Duplicate setter calls fail at compile time with understandable diagnostics.
- Docs explain when to compose Statum with an ordinary builder crate.

### 4. Compile-time performance

Compile-time cost should be visible and regression-tested enough to keep design
tradeoffs honest.

Acceptance checks:

- Compile benchmark command is documented.
- Reports include cold and warm `cargo check` measurements for plain and Statum
  fixtures.
- Reports include the Statum/plain ratio, not only raw milliseconds.
- Any strict-introspection benchmark claim states exactly which fixture and
  command produced it.

### 5. Flagship workflow and persistence companion

The document-approval workflow should be the main proof that Statum solves more
than simple construction. The event-log case study should remain the persistence
companion for projection-heavy systems.

Acceptance checks:

- The flagship shows state-specific data, legal transitions, typed
  rehydration, and graph output.
- The event-log companion shows the three boundaries: event log, projection row,
  typed machine.
- It explains what bugs remain runtime concerns and what bugs the type system
  removes.
- It includes a short "why not just an enum?" section.
- It points to the exact runnable showcase and test coverage.

## Current Next Tasks

1. Keep README positioning focused on typestate workflows and typed
   rehydration; avoid defensive LOC/repo-size rationale.
2. Add a diagnostics audit document and use it as the checklist for future
   stderr fixture improvements.
3. Add a builder UX positioning document so feature requests can be classified
   as Statum-owned vs. ordinary-builder-owned.
4. Add compile benchmark reporting docs before making performance claims.
5. Keep the document-approval flagship and event-log companion aligned with the
   README positioning.
