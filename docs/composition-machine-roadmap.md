# Composition Machine Roadmap

This file is the canonical roadmap for shifting Statum from declared journey
metadata toward typed composition machines as the source of workspace flow
truth.

## Summary

The target model is:

- local protocol legality stays in ordinary Statum machines
- workspace journey truth lives in ordinary `#[machine]` types marked
  `role = composition`
- direct child-machine composition is automatic from typed state payloads,
  machine fields, and transition parameters
- every transition site has first-class provenance available for exact
  introspection
- detached artifacts use one exact producer-side handoff surface instead of a
  separate narrative DSL
- the inspector, CLI, and graph bundle all project the same exact substrate
  while keeping heuristics separate

Current `journeys!`, `#[via(...)]`, and related narrative or attestation
surfaces stay in place during migration, then get trimmed once composition
machines reach parity.

## Locked Decisions

- Composition machines are ordinary machines with
  `#[machine(role = composition)]`, not a separate top-level macro.
- `#[transition]` should infer the machine from the inherent `impl` self type.
  Legacy `#[transition(Machine)]` stays only as a compatibility path during
  migration.
- Direct child-machine references in a composition machine are the primary
  exact composition surface.
- Transition-site provenance exists for every transition site, but normal user
  APIs stay lean. Provenance-carrying values appear only where exact
  cross-machine handoff needs them.
- Detached artifacts use a producer-side handoff declaration attached to a
  real transition, not a free-standing journey DSL.
- `#[machine_ref(...)]` stays for opaque references that cannot honestly be tied
  to one producer transition site.
- Heuristics remain TUI-only and never become exact export data.
- `#[present(...)]` is not a priority in this roadmap.

## Layering

Narrative layer:

- inspector atlas
- relationship cards
- path explorer
- gaps and migration guidance

Stage layer:

- composition-edge derivation
- path selection and grouping
- heuristic overlay and overlap suppression
- migration suggestions

Protocol-truth layer:

- ordinary protocol machines
- composition machines
- transition-site provenance
- detached handoff evidence
- exact graph export and `CodebaseDoc`

Leaf mechanics:

- renderers
- search and filters
- formatting
- snippets and labels

## Phases

### Phase 1: Foundation And Provenance

Checklist: [Phase 1 checklist](./checklists/composition-phase-1-foundation.md)

Status: complete

Goal:

- make composition a first-class exact concept without changing the normal
  machine programming model

Deliver:

- `MachineRole` with `protocol` and `composition`
- `#[machine(role = composition)]`
- `#[transition]` inference from the inherent `impl` self type
- compatibility path for legacy `#[transition(Machine)]`
- explicit transition-site provenance in linked metadata and exact export
- fail-closed authority rules and adversarial tests for the new provenance and
  role surfaces

Exit criteria:

- composition role is exported everywhere exact machine metadata is exported
- transition-site identity is strong enough to support later handoff and
  inspector drilldown work
- current exact exports remain exact-only and compatible

### Phase 2: Direct Child-Machine Composition

Checklist:
[Phase 2 checklist](./checklists/composition-phase-2-direct-composition.md)

Status: complete

Goal:

- make composition machines automatically define exact workspace flow whenever
  they directly carry child machines

Deliver:

- exact composition-edge derivation from child-machine payloads, machine
  fields, and transition parameters on composition machines
- composition-specific relation basis/detail in `CodebaseDoc`
- graph, CLI, and TUI projection of composition-owned exact relations
- path derivation that prefers composition-owned exact paths

Exit criteria:

- a composition machine with direct child-machine payloads is enough to define
  an exact top-level journey surface
- no separate journey DSL is required for those cases

### Phase 3: Detached Handoffs And Diagnostics

Checklist:
[Phase 3 checklist](./checklists/composition-phase-3-detached-handoffs.md)

Goal:

- cover exact cross-machine flow when composition crosses a detached artifact
  boundary instead of a direct child-machine value

Deliver:

- producer-side detached provenance reusing exact attested transition sites
- exact joining of detached artifacts from producer transition to composition
  consumer across state payloads, machine fields, and transition parameters
- continued support for `#[machine_ref(...)]` on opaque references
- exact compiler warnings for high-confidence typed orchestration smells
- inspector or CLI suggestions for weaker composition candidates

Exit criteria:

- detached handoffs participate in exact composition relations
- detached handoff route identities fail closed when producer metadata drifts on
  target state
- diagnostics push users toward composition modeling without weakening the
  exactness story

### Phase 4: Inspector, Atlas, And Path UX

Checklist:
[Phase 4 checklist](./checklists/composition-phase-4-inspector.md)

Goal:

- make composition machines the main way users understand a workspace

Deliver:

- composition-first home view in the inspector
- relationship cards that prefer composition-owned explanations
- path explorer that prefers composition paths, then raw exact graph, then
  heuristic fallback
- gaps view that shows what is still heuristic or still modeled through older
  compatibility surfaces
- graph bundle and JSON staying exact-only while the TUI layers narrative and
  heuristic views on top

Exit criteria:

- a user can understand the top-level workspace flow from composition machines
  before drilling into leaf protocol machines

### Phase 5: Downstream Validation And Migration

Checklist:
[Phase 5 checklist](./checklists/composition-phase-5-migration.md)

Goal:

- prove the model in a real consumer workspace before deleting old surfaces

Deliver:

- at least one real downstream migration using composition machines
- promotion of the highest-value detached handoffs and opaque references
- validation that exact workspace flow becomes substantially more complete than
  the current journey-plus-heuristic story
- migration guide from current `journeys!`, `#[via(...)]`, and
  `#[machine_ref(...)]` usage to the composition model

Exit criteria:

- a real workspace can expose its main journey map through composition
  machines, not just through declared journeys or heuristics

### Phase 6: Cleanup, Deprecation, And Docs Trim

Checklist:
[Phase 6 checklist](./checklists/composition-phase-6-cleanup.md)

Goal:

- trim Statum down after composition machines and detached handoffs have
  reached parity

Deliver:

- deprecation or removal of stale journey-first surfaces
- deprecation or removal of superseded attestation helpers and binder APIs
- cleanup of compatibility paths kept only for migration
- pruning of stale docs, examples, and TUI copy
- updated canonical docs that describe the composition model first

Exit criteria:

- the recommended mental model, public docs, examples, and inspector all point
  to the same composition-first story
- old APIs that no longer carry their weight are gone or clearly deprecated

## Cross-Phase Rules

- Exact exports stay exact-only through every phase.
- Heuristics stay out of `CodebaseDoc`, JSON, Mermaid, DOT, and PlantUML.
- Public docs must distinguish current shipped behavior from roadmap target
  behavior while migration is in progress.
- Cleanup happens only after a real downstream migration proves the new model.
