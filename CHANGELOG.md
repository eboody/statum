# Changelog

## v0.9.0 - 2026-06-09

Statum v0.9.0 is the first public release candidate after the roadmap integration pass. It turns the project from a macro prototype into a cohesive typestate workflow toolkit with compile-time contracts, optional introspection, rebuild diagnostics, graph metadata, CLI reporting, and real application showcases.

### Highlights

- Added stable graph/introspection metadata for states, transitions, transition sites, renderers, and graph linting.
- Added rebuild report surfaces for validator-driven reconstruction diagnostics, including batch report helpers behind explicit feature flags.
- Added transition telemetry helpers that expose stable low-cardinality labels without requiring a telemetry dependency.
- Added `cargo-statum` graph, docs, explain, agent-context, and graph-diff workflows for reviewing generated machine metadata.
- Expanded trybuild coverage for state, machine, transition, validator, strict-introspection, and feature-boundary contracts.
- Added generated/curated diagnostic documentation for compile-fail fixtures and feature-gated generated surfaces.
- Expanded examples into realistic Axum, SQLite, Tokio, Clap, serde JSON, event-log rebuild, job-runner, websocket-session, and deployment-pipeline showcases.
- Made example domain surfaces more semantic with typed IDs, receipts, tokens, close/failure reasons, and DTO-boundary promotion.
- Clarified feature-boundary truth: the main crate remains feature-free by default; examples opt into introspection when they render generated graph/report surfaces.
- Added repo-local Semantic Code Doctrine guidance for future development and agent-assisted reviews.

### Release notes

Publishable crates in this release are:

- `statum-core` v0.9.0
- `statum-macros` v0.9.0
- `statum` v0.9.0

`statum-examples` and `cargo-statum` remain workspace/public-source packages with `publish = false`.
