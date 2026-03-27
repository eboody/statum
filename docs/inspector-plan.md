# Statum Inspector Roadmap

This file tracks the remaining inspector work after the exact codebase viewer
and heuristic overlay shipped.

## Current Shipping Surface

Today `cargo statum-graph inspect /path/to/workspace` already provides:

- exact linked compiled machine topology
- exact declared validator-entry surfaces
- exact direct-construction availability per state
- exact cross-machine relations from state payloads, machine fields,
  transition parameters, `#[via(...)]` declarations, and nominal
  `#[machine_ref(...)]` declarations
- machine, relation, and detail panes over the linked `CodebaseDoc`
- search plus exact relation-kind and relation-basis filters
- heuristic-only, exact-only, and mixed lane toggles
- source-scanned heuristic machine-coupling overlay with explicit unavailable
  handling

The exact lane remains the only authoritative lane. The heuristic lane stays
useful but non-authoritative and TUI-only.

## Authority Contract

Exact lane:

- claimed authority surface:
  exact linked compiled machine topology, validator-entry surfaces,
  direct-construction availability, and exact relation detail
- actual observation point:
  `MachineIntrospection::GRAPH` plus linked compiled machine, validator-entry,
  relation, attested-route, and reference-type inventories
- fail-closed rule:
  unsupported exact cases must reject explicitly or contribute no exact
  metadata

Heuristic lane:

- claimed authority surface: none
- actual observation point:
  parsed raw source of reachable library module trees plus the selected
  packages' transition signatures and bodies
- fail-closed rule:
  unavailable or partial collection must be shown as unavailable rather than
  guessed

Runtime lane:

- claimed authority surface:
  exact executed transition paths from explicit runtime events and only the
  snapshots or projections an application explicitly provides

## Remaining Phases

### Phase 1: Replay MVP

Deliverables:

- runtime transition event protocol
- per-instance replay timelines
- timeline stepping
- graph and timeline selection sync

Success criteria:

- exact executed paths are replayable without snapshots

### Phase 2: Snapshot Support

Deliverables:

- snapshot protocol
- before and after view
- generic structural diff
- explicit unavailable state when no snapshot exists

Success criteria:

- data changes are inspectable when provided and visibly unavailable when not

### Phase 3: Composition Tree

Deliverables:

- parent and child machine tree
- attach and detach events
- nested replay stepping

Success criteria:

- users can step into and out of sub-machines without losing context

### Phase 4: Adapter Ergonomics And Polish

Deliverables:

- helper traits
- helper macros only if justified
- redaction hooks
- custom labels
- saved filters and layouts

Success criteria:

- one application can integrate without rebuilding the whole protocol

## Remaining Test Plan

Runtime tests:

- replay order
- missing event fields fail clearly
- stale or out-of-order sequences are rejected

Snapshot tests:

- missing snapshots
- redacted values
- structural diffs across maps and lists

Composition tests:

- attach and detach ordering
- nested machine stepping
- orphan child detection

Adversarial tests:

- replay with no snapshots
- composition events with unknown machine ids
- duplicate machine instance ids
- stale export and runtime protocol version mismatch

## Checklist

- [ ] Add replay session model
- [ ] Add timeline stepping
- [ ] Add snapshot protocol and generic structural diffing
- [ ] Add composition tree and nested replay navigation
- [ ] Add helper hooks and one sample application adapter
- [ ] Add replay, snapshot, and composition protocol tests

## Acceptance Criteria

The shipped inspector already answers:

- what machines exist in this workspace
- where a machine can be entered through validators
- which states are directly constructible
- what points at a machine, state, or transition
- why that exact relation exists

This roadmap is complete when the inspector can also answer:

- what executed at runtime
- what data changed
- where a machine sits in a parent and child runtime tree
