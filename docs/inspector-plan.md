# Statum Inspector Plan

This plan describes the shared inspector TUI for Statum.

The inspector is now codebase-first.

The first job is not replay. The first job is to make Statum's exact static
protocol surface navigable at workspace scale, then layer heuristic discovery
and runtime replay on top without mixing their authority levels.

## Goal

Build a reusable inspector in the spirit of `lazygit` that lets a developer:

- see what machines exist in a workspace
- inspect each machine's states and legal transitions
- inspect declared validator entry surfaces
- see which states are directly constructible
- see what points at a selected machine, state, or transition
- see why a relation exists

Later phases add:

- heuristic relation overlays
- replay of recorded runtime transitions
- snapshots and diffs
- parent and child composition trees

## Dependency

The exact static substrate is tracked separately in
[exact-static-relations-plan.md](/home/eran/code/statum/docs/exact-static-relations-plan.md).

That plan is the protocol-truth dependency for this one.

The inspector should consume that substrate. It should not re-derive exact
static relations for itself.

## Non-Goals

This plan does not try to:

- infer runtime values from static type information alone
- export heuristic relations as if they were exact
- infer parent and child runtime composition from module layout or naming
- promise exact data diffs without explicit snapshot or projection input
- replace the exact codebase export model with a TUI-only interpretation

## Authority Contract

Claimed authority surface in the exact lane:

- exact linked compiled machine topology
- exact declared validator-entry surfaces
- exact direct-construction availability per state
- exact static relations from the codebase export substrate

Actual observation point for the exact lane:

- `MachineIntrospection::GRAPH`
- linked compiled machine, validator-entry, relation, and reference-type
  inventories
- macro-expanded, cfg-pruned `#[machine]`, `#[transition]`, `#[validators]`,
  and `#[machine_ref]` items

Claimed authority surface in the heuristic lane:

- none

The heuristic lane is useful but non-authoritative. It may use broader source
or body analysis later, but it must stay visually and semantically separate
from exact relations.

Claimed authority surface in the runtime lane:

- exact executed transition paths from explicit runtime events
- exact displayed data only for snapshots or projections an application
  explicitly provides

Fail-closed rule:

If the inspector does not have the required exact, heuristic, or runtime
inputs, it must show that the information is unavailable. It must not guess.

## Layering

Narrative layer:

- the interactive inspector app and its panes

Stage layer:

- one workspace session model
- exact static selection state
- optional heuristic overlay state
- later replay and composition state

Protocol-truth layer:

- exact `CodebaseDoc` export
- later runtime transition events
- later runtime composition events
- later snapshot or projection inputs

Plain-function leaves:

- rendering helpers
- keybindings
- search and filtering
- layout and focus management
- label formatting

Duplication risks:

- separate graph models for exact export and the inspector
- separate relation semantics in renderers and the TUI
- mixing heuristic relations into the exact export contract

Locality risks:

- spreading selection and relation semantics across panes instead of one
  session model
- making the graph widget decide semantics that belong in the session or
  export model

Invariant-placement risks:

- computing exact relations from heuristic scans
- inferring executed paths from topology
- inferring data changes from type metadata alone

## Product Shape

The default TUI should use a stable multi-pane layout:

- left: workspace overview, machine list, and disconnected groups
- center: exact graph view or later timeline view
- right: machine, state, transition, and relation details for the current
  selection
- bottom: search results, filter status, and later runtime event logs

Core exact-lane views:

- workspace overview
- machine view
- relation view
- detail pane

Workspace overview should show:

- machine count
- disconnected groups
- exact machine summary edges
- filters for relation kind and provenance

Machine view should show:

- states
- transitions
- validator-entry nodes
- builder markers

Relation view should show:

- inbound exact relations for the current machine, state, or transition
- outbound exact relations for the current machine, state, or transition
- relation kind
- relation basis

Detail pane should show why a relation exists, for example:

- `state_payload`
- `machine_field`
- `transition_param`
- `validator_entry`
- `direct_construction_available`

## Exact And Heuristic Lanes

The inspector should expose two different static lanes.

Exact lane:

- backed by `CodebaseDoc`
- exported through Mermaid, DOT, PlantUML, and JSON
- default view when the inspector opens

Heuristic lane:

- optional and TUI-only
- separate filters and styling
- visible provenance for why each heuristic relation exists
- never merged into the exact export model

## Phases

### Phase 0: Exact Static Substrate

Source of truth:

- [exact-static-relations-plan.md](/home/eran/code/statum/docs/exact-static-relations-plan.md)

Status:

- exact static substrate done
- inspector UI work is the next open milestone

### Phase 1: Exact Codebase Viewer MVP

Deliverables:

- load one `CodebaseDoc`
- workspace overview
- machine view
- validator-entry display
- visible builder markers
- exact machine summary edges

Success criteria:

- graph-only mode answers what machines exist and how they statically connect

### Phase 2: Exact Relation Navigation

Deliverables:

- relation pane
- inbound and outbound navigation
- provenance detail pane
- exact-lane search and filtering

Success criteria:

- the user can answer what points at this and why

### Phase 3: Heuristic Overlay Lane

Deliverables:

- optional heuristic relation collector
- separate styling from the exact lane
- provenance display for heuristic relations
- explicit unavailable state when heuristic analysis cannot run

Success criteria:

- users can opt into broader discovery without weakening the exact lane

### Phase 4: Replay MVP

Deliverables:

- runtime transition event protocol
- per-instance replay timelines
- timeline stepping
- graph and timeline selection sync

Success criteria:

- exact executed paths are replayable without snapshots

### Phase 5: Snapshot Support

Deliverables:

- snapshot protocol
- before and after view
- generic structural diff
- explicit unavailable state when no snapshot exists

Success criteria:

- data changes are inspectable when provided and visibly unavailable when not

### Phase 6: Composition Tree

Deliverables:

- parent and child machine tree
- attach and detach events
- nested replay stepping

Success criteria:

- the user can step into and out of sub-machines without losing context

### Phase 7: Adapter Ergonomics And Polish

Deliverables:

- helper traits
- helper macros only if justified
- redaction hooks
- custom labels
- saved filters and layouts

Success criteria:

- one application can integrate without rebuilding the whole protocol

## Testing Plan

Exact-lane tests:

- workspace with multiple machines
- disconnected groups
- exact machine summary edges
- builder markers
- relation-pane lookup for inbound and outbound edges
- provenance details for each exact relation kind

Heuristic-lane tests:

- heuristic relations are visually distinct
- heuristic provenance is visible
- heuristic results never appear in exact export output
- unavailable heuristic state is explicit

Runtime tests:

- replay order
- missing event fields fail clearly
- stale or out-of-order sequences are rejected

Composition tests:

- attach and detach ordering
- nested machine stepping
- orphan child detection

Snapshot tests:

- missing snapshots
- redacted values
- structural diffs across maps and lists

Adversarial tests:

- exact lane with no heuristic overlay
- heuristic lane with no runtime replay
- runtime replay with no snapshots
- composition events with unknown machine ids
- duplicate machine instance ids
- stale export and runtime protocol version mismatch

## Checklist

- [x] Create the exact static substrate plan
- [x] Make the inspector plan depend on that substrate
- [x] Render builder markers in the exact graph backends
- [x] Derive and render exact machine summary edges
- [ ] Add workspace overview to the TUI
- [ ] Add machine view with validators and builder markers
- [ ] Add relation pane with inbound and outbound navigation
- [ ] Add provenance detail pane
- [ ] Add exact-lane search and filters
- [ ] Add separate heuristic overlay lane
- [ ] Add replay session model
- [ ] Add timeline stepping
- [ ] Add snapshot protocol and generic structural diffing
- [ ] Add composition tree and nested replay navigation
- [ ] Add helper hooks and one sample application adapter
- [x] Add exact-lane tests for builder markers, summary edges, and relation
      provenance
- [ ] Add heuristic-lane separation tests
- [ ] Add replay, snapshot, and composition protocol tests

## Acceptance Criteria

This plan is working when the inspector can answer:

- what machines exist in this workspace
- where can this machine be entered through validators
- which states are directly constructible
- what points at this machine, state, or transition
- why that relation exists

Later phases extend that to:

- what executed at runtime
- what data changed
- where this machine sits in a parent and child runtime tree
