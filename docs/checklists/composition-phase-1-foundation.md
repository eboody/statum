# Composition Roadmap Checklist: Phase 1

Goal: foundation and provenance.

- [x] Add `MachineRole` to linked machine metadata, `CodebaseDoc`, and JSON.
- [x] Extend `#[machine]` with `role = composition`.
- [x] Make `#[transition]` infer the machine from the inherent `impl` self
      type.
- [x] Keep legacy `#[transition(Machine)]` working during migration.
- [x] Mark legacy `#[transition(Machine)]` as a compatibility path with a
      deprecation plan.
- [x] Expose transition-site provenance strongly enough for exact export and
      inspector drilldown.
- [x] Add exact export tests for composition role round-tripping.
- [x] Add adversarial tests for `#[cfg]`, macro-generated items, `include!`,
      and duplicate transition-name pressure.
- [x] Update roadmap and migration docs for the new role and transition
      direction.

Acceptance gate:

- [x] Composition role is available everywhere exact machine metadata is used.
- [x] Transition-site provenance is exact enough to support later phases
      without body inspection.
