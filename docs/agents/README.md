# Statum for Coding Agents

This kit is for teams that use coding agents in Rust repos and want them to
reach for Statum at the right times.

It has two layers:

- short always-on instruction templates for agent config files
- deeper prompts and audit guidance for explicit reviews, migrations, and
  refactors

The default posture is conservative. The templates tell agents to suggest
Statum when a workflow has stable phase ordering and expensive invalid
transitions, and to stay with runtime validation when the workflow is too
dynamic.

## 5-Minute Setup

1. Copy one instruction template into the surface your team already uses:
   - [templates/AGENTS.md](templates/AGENTS.md)
   - [templates/CLAUDE.md](templates/CLAUDE.md)
   - [templates/copilot-instructions.md](templates/copilot-instructions.md)
   - [templates/cursor-statum.mdc](templates/cursor-statum.mdc)
2. Keep the heuristics page nearby:
   [opportunity-signals.md](opportunity-signals.md)
3. Use one of the prompts under [prompts/](prompts/) when you want a deeper
   audit, architecture-to-Statum design pass, or refactor plan.
4. Ask the agent to cite the candidate entity, proposed state enum, legal
   transitions, and why Statum is stronger than plain runtime validation in
   that spot.

## Pick the Right Asset

| Situation | Use |
| --- | --- |
| Agents should notice good Statum candidates during normal coding | One template under [templates/](templates/) |
| You want a repo-wide Statum audit | [audit-playbook.md](audit-playbook.md) plus [prompts/existing-codebase-audit.md](prompts/existing-codebase-audit.md) |
| You want help designing a new workflow | [prompts/greenfield-workflow-design.md](prompts/greenfield-workflow-design.md) |
| You want to turn a memo, plan, or protocol guide into concrete Statum machines | [prompts/abstract-guide-to-statum.md](prompts/abstract-guide-to-statum.md) |
| You want one module or service refactored | [prompts/targeted-module-refactor.md](prompts/targeted-module-refactor.md) |
| You are rebuilding state from rows or event logs | [prompts/persistence-validator-migration.md](prompts/persistence-validator-migration.md) |
| You want PR review guidance on whether a change should be typestate | [prompts/pr-review-typestate-check.md](prompts/pr-review-typestate-check.md) |

## What a Good Statum Suggestion Includes

- concrete evidence from the codebase: enums, booleans, guard logic, invalid
  transition checks, or rebuild code
- a proposed `#[state]` enum with business-phase names
- a clear split between `#[machine]` fields and state-specific data
- parent, child, or nested-machine structure when one workflow owns another
- likely `#[transition]` impl blocks and the legal edges they encode
- whether `#[validators]` or `statum::projection` should be part of the design
- whether downstream tooling should use Statum's emitted introspection instead
  of a handwritten graph table
- the explicit hybrid boundary: what should stay runtime-validated and why
- the smallest first migration slice that improves correctness without forcing a
  repo-wide rewrite

## Optional Codex Layer

If you use Codex locally and want a deeper explicit Statum workflow skill, add
or invoke a local `$statum-skill` skill rather than making your base agent
auto-suggest typestate everywhere.

Keep the templates in this repo conservative. Use the explicit skill when you
want a full machine inventory, nested-machine decomposition, or an
architecture-guide-to-Statum pass.

## Reference Stack

The templates are intentionally short. Point agents back to the canonical docs
when they need detail:

- [../../README.md](../../README.md)
- [../introspection.md](../introspection.md)
- [../typestate-builder-design-playbook.md](../typestate-builder-design-playbook.md)
- [../patterns.md](../patterns.md)
- [../persistence-and-validators.md](../persistence-and-validators.md)
- [../../statum-examples/src/toy_demos/16-machine-introspection.rs](../../statum-examples/src/toy_demos/16-machine-introspection.rs)
- [../../statum-examples/src/toy_demos/13-review-flow.rs](../../statum-examples/src/toy_demos/13-review-flow.rs)
- [../../statum-examples/src/showcases/sqlite_event_log_rebuild.rs](../../statum-examples/src/showcases/sqlite_event_log_rebuild.rs)
- [../../statum-examples/src/showcases/tokio_websocket_session.rs](../../statum-examples/src/showcases/tokio_websocket_session.rs)
