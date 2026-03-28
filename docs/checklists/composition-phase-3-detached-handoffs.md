# Composition Roadmap Checklist: Phase 3

Goal: detached handoffs and diagnostics.

- [ ] Add a producer-side detached handoff declaration on real transition
      sites.
- [ ] Record producer machine, source state, and transition provenance for
      detached handoffs.
- [ ] Join detached handoff consumers in composition state payloads, machine
      fields, and transition parameters.
- [ ] Keep `#[machine_ref(...)]` for opaque references not tied to one producer
      transition.
- [ ] Decide and document the boundary between detached handoffs and opaque
      machine references.
- [ ] Add exact compiler warnings for high-confidence typed orchestration
      smells.
- [ ] Add inspector or CLI suggestions for weaker composition candidates that
      are not strong enough for compiler warnings.
- [ ] Add adversarial tests for unknown producer machine, source state, and
      transition, plus duplicate declaration pressure.
- [ ] Add migration guidance from `#[via(...)]` and `Attested<T, Via>` toward
      the new detached-handoff model.

Acceptance gate:

- [ ] Detached artifacts can participate in exact composition relations without
      pushing users back into a narrative DSL.
