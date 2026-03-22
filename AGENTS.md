# Repository Guidelines

## Mandatory Skills
- Use `ai-slop-guard` for every task that produces user-facing prose.
- Use `crimee-audit` after any task that creates or modifies code.
- Use `semantic-authority-audit` for macros, code generation, introspection, schema or policy extraction, docs generation, static analysis, or any work claiming exact, authoritative, exhaustive, or source-of-truth semantics.

## Semantic Authority Rule
- Treat words like `exact`, `source of truth`, `authoritative`, `exhaustive`, `no drift`, and `exact branch alternatives` as proof obligations.
- Before shipping such a feature, state the observation point the implementation actually uses: raw source, parsed AST, cfg-pruned AST, expanded items, type-checked items, runtime values, or persisted state.
- If the observation point is weaker than the public claim, the feature must fail closed or the claim must be narrowed.
- Happy-path tests are not sufficient for authority features. Add adversarial cases for constructs that differ across observation stages, including `#[cfg]`, macro-generated items, `include!`, and duplicate-id pressure where relevant.
- Closeout for authority features must state: claimed authority surface, actual observation point, unsupported cases rejected or still open, and adversarial tests added.

## Project Structure & Module Organization
This is a Rust workspace with four crates:
- `statum/` public API crate.
- `statum-core/` core types and shared logic.
- `statum-macros/` proc-macro crate with tests in `statum-macros/tests/` and UI fixtures in `statum-macros/tests/ui/`.
- `statum-examples/` example crate with modules in `statum-examples/src/examples/`.
Other notable paths: `scripts/` for release helpers and `logo.png` for branding.

## Build, Test, and Development Commands
- `cargo build --workspace`: build all workspace crates.
- `cargo test --workspace`: run all tests.
- `cargo test -p statum-macros`: focuses on macro tests (includes trybuild UI cases).
- `cargo run -p statum --example 01-setup`: run a specific example.
- `cargo fmt` / `cargo clippy --workspace`: formatting and lint checks (use before PRs).

## Coding Style & Naming Conventions
- Rustfmt defaults (4-space indent) are expected; keep code formatted via `cargo fmt`.
- Naming: `snake_case` for modules and functions, `CamelCase` for types/traits, `SCREAMING_SNAKE_CASE` for constants.
- Maintain proc-macro file layout in `statum-macros/src/` (e.g., `state.rs`, `machine.rs`, `validators.rs`).
- Macro diagnostics should name the relevant state enum (e.g., `TaskState`) to keep the DSL errors clear.

## Rewrite Guidance (New API)
- Treat the README as the canonical API spec once updated; keep examples and macros in sync with it.
- Prefer type-driven validation (state data type) over function-body inspection.
- Keep macro error messages precise, actionable, and scoped to the correct enum/machine.
- Favor deterministic macro behavior even if it requires extra scanning or caching.

## Style and Patterns Observed
- API aims for minimal boilerplate and strong compile-time guarantees.
- `#[state]` enums generate per-variant marker types and a trait for bounds.
- `#[machine]` types track `marker` and `state_data` fields; transitions consume `self`.
- `#[validators]` maps persistent data to machine states via `is_*` methods.
- Examples emphasize clarity and progressive complexity; keep code in examples simple and explicit.

## Testing Guidelines
- Unit and integration tests run via `cargo test`.
- Macro compile tests use `trybuild`; add new cases as `.rs` files under `statum-macros/tests/ui/` with matching `.stderr` expectations when relevant.

## Toolchain and Features
- The repo pins stable via `rust-toolchain.toml`; use the pinned stable toolchain when building/testing.

## Commit & Pull Request Guidelines
- Recent history mixes conventional prefixes (`build:`, `refactor(scope):`) and informal messages; prefer `type(scope): short summary` when possible.
- PRs should include a clear description, linked issues if applicable, and tests/examples updates when behavior changes.
