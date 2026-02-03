# Statum Rewrite Plan

## Goals
- Deliver a new, stable public API that keeps the macro ergonomics while tightening correctness and diagnostics.
- Keep the runtime core small, explicit, and easy to reason about.
- Provide clear migration guidance from the current API.
- Update examples, docs, and tests to reflect the new API surface and behaviors.

## Success Criteria
- All workspace crates build and tests pass on nightly.
- New API is documented in the README and has at least one end-to-end example per major feature.
- Macro UI tests cover all expected diagnostics and success paths.
- Migration guide covers: state definition, machine definition, transitions, validators, serde.
- No regression in compile-time safety compared to main.

## Milestones

### 1) Define the Target API (spec-first)
- Lock down the new attribute macros and derive requirements.
- Decide on constructor/builder ergonomics and any breaking changes.
- Decide transition API rules for data-bearing states (type-based, not body-based).
- Decide validators API shape and async support.
- Produce a concise API spec in `docs/` or README.

Deliverables:
- `docs/new-api.md` (or README section) with examples and rules.
- A migration table: old API -> new API changes.

### 2) Core Types and Runtime Semantics
- Verify `statum-core` types match the new API expectations.
- Confirm trait names, bounds, and visibility strategy.
- Ensure serde behavior is consistent and documented.

Deliverables:
- Updated `statum-core` API (if changes are needed).
- Tests for core traits and data-bearing state behavior.

### 3) Macro Implementation
- Deterministic loading: state + machine discovery is reliable across modules.
- Diagnostics: all errors refer to the correct state enum / machine type.
- Transition codegen uses state data type information (no body inspection).
- Validator enforcement is explicit and consistent with the spec.
- Rust-analyzer shims compile cleanly and are isolated.

Deliverables:
- Updated `statum-macros` with macro UI tests.
- `trybuild` cases for every major rule and error.

### 4) Examples and Documentation
- Move examples to `statum-examples` and ensure they compile.
- Rewrite README Quick Start and API reference to the new API.
- Add a migration guide and highlight breaking changes.

Deliverables:
- `statum-examples` aligned to the new API.
- README updated and consistent across crate copies.

### 5) Validation and Release Prep
- Run `cargo test --workspace` and `cargo test -p statum-macros`.
- Check formatting and clippy.
- Finalize versioning and release notes.

Deliverables:
- Passing CI and a clear release checklist.

## Current Gaps (from this branch)
- Core/public API crates are mostly unchanged; rewrite is largely macro-side.
- Examples are partially migrated to a new crate but need review vs API spec.
- README still describes the old API and transition rules.

## Suggested First Steps
1. Write the new API spec (short, explicit). Decide which behaviors are breaking.
2. Update README Quick Start to match the new API.
3. Align macro diagnostics and UI tests to the spec.
4. Port examples to the spec and validate with tests.

