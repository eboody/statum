# Composition Roadmap Checklist: Phase 5

Goal: downstream validation and migration.

- [x] Pick at least one real consumer workspace as the migration proving
      ground.
      `statum-examples` is now the proving ground for this phase.
- [x] Convert the highest-value declared journeys, or the nearest
      journey-like workflow examples when no declared journeys exist, into
      composition machines where the flow is truly protocol truth.
      The old hierarchical-machine demo now uses `#[machine(role = composition)]`.
- [x] Promote detached handoffs and opaque references only where exactness is
      honest.
      The new composition example keeps direct child-machine orchestration for
      review and one detached attested publication handoff.
- [x] Verify the exact graph bundle shows more of the main workflow than the
      current journey-plus-heuristic story.
      `cargo statum-graph export statum-examples` now exports composition
      summary edges for both direct child-machine and detached-handoff paths.
- [x] Verify the inspector composition view is materially more useful than the
      old journey-first view.
      `cargo statum-graph inspect statum-examples` now opens on `Composition`
      with the `DocumentFlow` path explorer as the top-level workspace story.
- [x] Write a migration guide from `journeys!`, `#[via(...)]`, and older
      attested helpers.
      See [composition-migration.md](../composition-migration.md).
- [x] Update examples to show the composition model in practice.
      `example_18_composition_machine.rs` is the composition-first proving
      ground, and `11-hierarchical-machines.rs` now uses the composition role.

Acceptance gate:

- [x] A real workspace can expose its main journey map through composition
      machines and exact handoffs, not just through declared journeys.
