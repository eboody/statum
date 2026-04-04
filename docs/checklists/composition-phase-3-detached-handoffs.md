# Composition Roadmap Checklist: Phase 3

Goal: detached handoffs and diagnostics.

- [x] Reuse the existing producer-side attested transition sites as the
      detached handoff provenance surface.
- [x] Record producer machine, source state, target state, and transition
      provenance for detached handoffs.
- [x] Join detached handoff consumers in composition state payloads, machine
      fields, and transition parameters.
- [x] Keep `#[machine_ref(...)]` for opaque references not tied to one producer
      transition.
- [x] Decide and document the boundary between detached handoffs and opaque
      machine references.
- [x] Add exact `cargo statum-graph suggest` warnings for high-confidence
      typed orchestration smells.
- [x] Add inspector and CLI suggestions for weaker composition candidates that
      are not strong enough for exact warnings.
- [x] Add adversarial tests for unknown producer machine, source state, target
      state, and transition, plus duplicate declaration pressure.
- [x] Add migration guidance from `#[via(...)]` and `Attested<T, Via>` toward
      detached handoffs on composition machines.

Acceptance gate:

- [x] Detached artifacts can participate in exact composition relations without
      pushing users back into a narrative DSL.
