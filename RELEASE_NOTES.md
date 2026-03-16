# Release Notes

## v0.5.5 (2026-03-16)

### Changes
- Restored ergonomic `#[machine]` field type handling so imported aliases, renamed imports, and module aliases no longer need to be rewritten as fully qualified paths.

## v0.5.4 (2026-03-15)

### Changes
- Expanded `macro_registry` with a reusable query layer for module-aware item discovery and typed registry lookup results.
- Refactored `statum-macros` to use the shared `macro_registry` query and named-lookup APIs instead of local duplicate resolution logic.
- Tightened proc-macro module-path resolution so metadata loading fails explicitly when the call-site module cannot be determined.
- Refreshed the `module_path_extractor` and `macro_registry` READMEs around real proc-macro workflows and public entry points.

## v0.5.3 (2026-03-12)

### Changes
- Added a `docs/agents/` adoption kit with copyable agent-instruction templates, audit heuristics, and targeted prompt packs for Statum-friendly workflows.
- Linked the agent kit from the root README and the published `statum` crate README.
- Expanded docs link validation to recurse through nested `docs/` content so the agent docs stay covered.

## v0.5.2 (2026-03-11)

### Changes
- Corrected crate metadata for docs.rs links and marked `statum-examples` as non-publishable.
- Expanded docs link validation to include the root README and `docs/*.md`.
- Hardened publish preflight so already-published versions fail early and downstream crates use package inspection when dry-runs are impossible before upstream publish.

## v0.5.1 (2026-03-10)

### Changes
- Expanded the public rustdoc surface for `statum` and `statum-core`.
- Added runnable crate-level examples for the root API pages.
- Clarified stable-toolchain guidance in internal docs.

## v0.5.0 (2026-03-10)

### Changes
- Cleaned up the public API around `machine::State`, `into_machine()`, `.into_machines()`, and `.into_machines_by(...)`.
- Added crate-level advanced traits such as `CanTransitionMap`.
- Added `statum::projection` for event-log projection before typed rehydration.
- Reworked macro diagnostics and UI coverage for more informative editor errors.
- Split examples into toy demos plus showcase apps, including Axum, CLI, worker, event-log, and protocol examples.

## v0.3.5 (2026-02-03)

### Changes
- Added `into_machine()` as the preferred validators builder entry point. `machine_builder()` remains for compatibility.
- Updated README examples to use `into_machine()` and `statum::Result`.

## v0.3.4 (2026-02-03)

### Changes
- Added `statum::Result<T>` as a convenience alias for `Result<T, statum::Error>`.
- README updated to use enum-based validator matching and `statum::Result`.

## v0.3.3 (2026-02-03)

### Changes
- Validators now generate a machine-scoped module alias for matching superstates.
- README updated to use the new module alias and enum-based status matching.

## v0.3.2 (2026-02-03)

### Changes
- README refreshed with enum-based validator examples and links to compiling examples/tests.

## v0.3.1 (2026-02-03)

### Changes
- Removed the `serde` feature and dependency; derives on `#[state]`/`#[machine]` are now fully user-driven.
- Bumped internal versions and publish metadata.
- Updated examples/docs to remove Serde usage.

## v0.3.0 (2026-02-03)

This release is a **major rewrite** focused on a cleaner typestate API, improved macro diagnostics, and a more complete example/test suite.

### Highlights
- **New examples crate**: examples moved into `statum-examples/` with runnable tests.
- **Macro diagnostics overhaul**: clearer, state-named errors for common misuse cases.
- **Deterministic macro loading**: more stable macro metadata resolution across modules.
- **Validators flow**: improved reconstruction ergonomics with a generated `{Machine}SuperState` for matching.
- **Robust module path extraction**: handles multi-module files more reliably.
- **Expanded test coverage**: trybuild UI cases + stress tests for API permutations.

### Breaking Changes / Migration
- **Examples moved**: use `statum-examples` instead of `statum/examples`.
- **Validator superstate**: reconstruction returns `{Machine}SuperState` (match on variants).
- **Docs restructured**: new rewrite docs + migration guide under `docs/`.

### Getting Started
```toml
[dependencies]
statum = "0.3.0"
```

Run examples/tests:
```bash
cargo test -p statum-examples
```

### Notes
If you were relying on older macro error strings or example paths, update to the new docs and examples layout.
