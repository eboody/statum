# Release Notes

## v0.3.0 (2026-02-03)

This release is a **major rewrite** focused on a cleaner typestate API, improved macro diagnostics, and a more complete example/test suite.

### Highlights
- **New examples crate**: examples moved into `statum-examples/` with runnable tests.
- **Macro diagnostics overhaul**: clearer, state‑named errors for common misuse cases.
- **Deterministic macro loading**: more stable macro metadata resolution across modules.
- **Validators flow**: improved reconstruction ergonomics with a generated `{Machine}SuperState` for matching.
- **Robust module path extraction**: handles multi‑module files more reliably.
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
