# Repository Guidelines

## Mandatory Skills
- Use `ai-slop-guard` for every task that produces user-facing prose.
- Use `crimee-audit` after any task that creates or modifies code.
- Use `semantic-authority-audit` for macros, code generation, introspection, schema or policy extraction, docs generation, static analysis, or any work claiming exact, authoritative, exhaustive, or source-of-truth semantics.

## Semantic Authority Rule
Treat words like `exact`, `source of truth`, `authoritative`, `exhaustive`, `no drift`, and `exact branch alternatives` as proof obligations.
- Before shipping such a feature, state the observation point it actually uses: raw source, parsed AST, cfg-pruned AST, expanded items, type-checked items, runtime values, or persisted state.
- If the observation point is weaker than the public claim, fail closed or narrow the claim.
- Happy-path tests are not sufficient. Add adversarial cases that differ across observation stages: `#[cfg]`, macro-generated items, `include!`, duplicate-id pressure.
- Closeout must state: claimed authority surface, actual observation point, unsupported cases rejected or still open, adversarial tests added.

## Workspace Layout
Rust workspace (`resolver = "2"`) with four published-or-internal crates:
- `statum/` - public API facade. Re-exports `statum-core` types and `statum-macros` proc macros. Has `strict-introspection` feature that toggles `statum-macros/strict-introspection`.
- `statum-core/` - runtime types (`Error`/`Result`, projection, introspection registry via `linkme`).
- `statum-macros/` - proc-macro crate (`#[state]`, `#[machine]`, `#[transition]`, `#[validators]`). See `statum-macros/ARCHITECTURE.md` for required subsystem layout and dependency direction (`source/` must not depend on `state`/`machine`/`transition`/`validators`).
- `statum-examples/` - `publish = false`. Non-obvious split:
  - `src/toy_demos/` and `src/showcases/` are library modules.
  - `src/bin/*.rs` are runnable binaries that call into showcases.
  - There are **no `[[example]]` targets**; do not use `cargo run --example`.

`benchmarks/compile/{plain,statum}-fixture` are deliberately `exclude`d from the workspace - do not treat them as workspace members or add them to CI paths.

## Toolchain
- `rust-toolchain.toml` pins `stable`. Workspace `rust-version = "1.93"` is the real MSRV and is enforced in CI via a `1.93.1` job.
- Edition varies per crate: `statum` and `statum-core` use `edition = "2021"`, `statum-macros` and `statum-examples` use `edition = "2024"`. Keep that when adding new code.
- `Cargo.lock` is gitignored (this repo ships only libraries).

## Commands
Run the local CI-parity closeout gate before closing implementation tasks:

```
bash scripts/check_ci_parity.sh
```

That script runs, in order: `cargo modum check --root . --mode warn`,
`cargo fmt --all --check`, README/docs link checks, escape-hatch and diagnostics
coverage audits, any `scripts/check_*diagnostic*.sh` or `scripts/check_*graph*.sh`
audit scripts, clippy, macro UI tests both without and with
`strict-introspection`, workspace tests, workspace hygiene, and rustdoc with
warnings denied.

Focused variants:
- Macro UI/compile tests only: `cargo test -p statum-macros`.
- Single example binary: `cargo run -p statum-examples --bin axum-sqlite-review` (see `statum-examples/src/bin/` for the list).
- Compile-time benchmark: `bash scripts/benchmark_compile.sh [--mode cold|warm|both]`.
- Runtime benchmark: `cargo bench -p statum --bench runtime`.

CI also runs `cargo modum check --root . --mode warn` (requires `modum` installed; see `[package.metadata.modum]` in the per-crate `Cargo.toml`s) and `cargo deny check` / `cargo audit`.

## `strict-introspection` Feature
`statum-macros/tests/macro_errors.rs` gates different trybuild fixture sets on `cfg(feature = "strict-introspection")`. When adding a UI test:
- Register the `.rs` path explicitly in `macro_errors.rs` under the right `#[cfg]` arm. Files under `tests/ui/` are **not** auto-picked.
- Match the filename prefix to intent: `invalid_*` -> `compile_fail`, `valid_*` -> `pass`, `strict_*` -> strict-only arms.
- `.stderr` expectations are required for `compile_fail` cases. Regenerate with `TRYBUILD=overwrite cargo test -p statum-macros`.
- Run the suite once per feature flag combination you touch: plain and `--features strict-introspection`.

## Macro Development Rules
- Respect the dependency direction in `statum-macros/ARCHITECTURE.md`: emission code must not perform fresh source scans; diagnostics must not broaden fail-closed behavior.
- Prefer type-driven validation (state data type) over function-body inspection.
- Macro diagnostics must name the relevant state enum / machine so errors are attributable (e.g. "TaskState", not "this enum").
- Favor deterministic macro behavior even if it costs extra scanning or caching.
- `statum-macros/wip/` is scratch space. `scripts/check_workspace_hygiene.sh` fails CI if anything other than `.gitignore` lives there; clean it before committing.

## Publishing
`scripts/check_publish_dry_run.sh` enforces publish order: `statum-core -> statum-macros -> statum`. All three share `version.workspace = true` from root `Cargo.toml`; bump there, not per-crate. Use `scripts/update_version.rs` (via `cargo script`).

## Testing Notes
- Integration tests under `statum-examples/tests/` dispatch into `toy_demos`/`showcases` modules; add new demos to `src/toy_demos/mod.rs` (or `showcases/mod.rs`) **and** a test entry, otherwise they will not run.
- `scripts/check_readme_links.sh` validates relative links in every root-level `README.md`, per-crate `README.md`, and all `docs/**/*.md`. Broken links fail CI.
- Workspace hygiene also rejects unexpected empty top-level directories and zero-byte dotfiles (`.codex` and `.gitignore` are the only allowed exceptions).

## Style
Rustfmt defaults (4-space). `snake_case` modules/functions, `CamelCase` types/traits, `SCREAMING_SNAKE_CASE` constants. Treat the root `README.md` as the canonical API spec and keep examples/macros aligned with it.

## Commits & PRs
Prefer `type(scope): short summary` (history mixes conventional prefixes with informal messages). PRs should update examples or tests when behavior changes.

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **statum** (5469 symbols, 9862 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/statum/context` | Codebase overview, check index freshness |
| `gitnexus://repo/statum/clusters` | All functional areas |
| `gitnexus://repo/statum/processes` | All execution flows |
| `gitnexus://repo/statum/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
