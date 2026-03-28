# Composition Roadmap Checklist: Phase 1

Goal: foundation and provenance.

- [ ] Add `MachineRole` to linked machine metadata, `CodebaseDoc`, and JSON.
- [ ] Extend `#[machine]` with `role = composition`.
- [ ] Make `#[transition]` infer the machine from the inherent `impl` self
      type.
- [ ] Keep legacy `#[transition(Machine)]` working during migration.
- [ ] Mark legacy `#[transition(Machine)]` as a compatibility path with a
      deprecation plan.
- [ ] Expose transition-site provenance strongly enough for exact export and
      inspector drilldown.
- [ ] Add exact export tests for composition role round-tripping.
- [ ] Add adversarial tests for `#[cfg]`, macro-generated items, `include!`,
      and duplicate transition-name pressure.
- [ ] Update roadmap and migration docs for the new role and transition
      direction.

Acceptance gate:

- [ ] Composition role is available everywhere exact machine metadata is used.
- [ ] Transition-site provenance is exact enough to support later phases
      without body inspection.
