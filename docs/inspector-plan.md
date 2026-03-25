# Statum Inspector Plan

This plan describes a reusable inspector for Statum machines: a terminal UI in
the spirit of `lazygit` that can inspect static machine topology, replay
recorded runtime transitions, step into sub-machine composition, and show data
changes when an application provides explicit snapshot inputs.

## Goal

Build a shared inspector stack that lets Statum do as much as possible by
default while keeping the remaining gaps explicit and app-owned.

Statum should own:

- static graph inspection
- replay and navigation over recorded traces
- the generic inspector protocol
- the generic TUI shell
- generic filtering, selection, stepping, and tree navigation

Applications should own:

- machine instance identity
- runtime transition events
- parent and child machine composition events
- before and after snapshots
- domain projections, redaction, and custom diffs

## Non-Goals

This plan does not try to:

- infer runtime values from type information alone
- infer parent and child machine composition from module layout or naming
- promise exact data diffs for arbitrary domain types without explicit
  projection or snapshot hooks
- replace existing introspection graph APIs

## Recommendation

Implement this as a capability-based inspector, not as a best-effort analyzer.

The shared Statum side should provide a stable protocol and a generic
navigation model. Each codebase should opt into richer inspection by
implementing small explicit adapters.

That keeps the authority boundary clear:

- Statum owns topology and generic replay behavior.
- Applications own runtime facts and domain views.

## Authority Contract

Claimed authority surface:

- exact machine-local topology from Statum graph data
- exact executed transition path from explicit runtime events
- exact displayed data only for snapshots or projections an application
  explicitly provides

Actual observation points:

- static topology: `statum::MachineIntrospection::GRAPH`
- executed path: recorded runtime transition and composition events
- data views: recorded snapshots or app-provided projection hooks

Unsupported unless provided explicitly:

- runtime-selected branch choice without runtime events
- sub-machine composition without composition events
- exact before and after data diffs without snapshots or a projection surface

Fail-closed rule:

If the inspector does not have the required runtime input, it must show that
the information is unavailable. It must not guess.

## Layering

Narrative layer:

- the interactive inspector app and its panes

Stage layer:

- replay session model
- selection state
- machine tree state
- timeline cursor

Protocol-truth layer:

- static graph export
- runtime transition events
- runtime composition events
- runtime snapshots or projections

Plain-function leaves:

- diff formatting
- tree expansion
- keyboard bindings
- filtering
- search
- rendering helpers

Duplication risks:

- separate graph models for static export and inspector replay
- separate composition models in app code and inspector code
- custom per-app event formats with no shared protocol

Locality risks:

- spreading replay state across TUI widgets instead of one session model
- letting rendering code decide semantics that belong in the replay model

Invariant-placement risks:

- trying to compute executed path from topology
- trying to compute data change from type metadata
- trying to compute nested composition from static naming conventions

## Product Shape

The default TUI should use a stable multi-pane layout:

- left: machine tree and sub-machine tree
- center: graph view or timeline view
- right: state, transition, and data details for the current selection
- bottom: event log, search results, and filter status

Core interactions:

- select machine instance
- step forward and backward through transitions
- jump to parent or child machine
- switch between graph and timeline
- inspect legal next transitions for the selected point
- inspect before and after snapshots when available

## Capability Model

### Level 0: Graph Only

Inputs:

- static graph data

Supports:

- states
- edges
- roots
- legal next transitions

### Level 1: Trace Replay

Inputs:

- graph data
- runtime transition events with machine instance ids

Supports:

- exact executed path replay
- timeline stepping
- per-instance navigation

### Level 2: Snapshot Inspection

Inputs:

- graph data
- transition events
- before and after snapshots or inspectable projections

Supports:

- per-step data inspection
- before and after diff views

### Level 3: Composition Tree

Inputs:

- graph data
- transition events
- composition events
- optional snapshots

Supports:

- parent and child machine tree
- step into and out of sub-machine timelines
- correlated replay across nested machines

### Level 4: Rich Domain Adapters

Inputs:

- all of the above
- app labels, redaction, custom diff formatting, and external ids

Supports:

- human-friendly labels
- redacted views
- richer diffs
- links to domain objects and external traces

## Crate Shape

Recommended crates:

- `statum-inspect-core`
- `statum-inspect-tui`
- `statum-inspect-json`

Optional integration in `statum`:

- feature-gated helpers for transition event emission
- feature-gated helpers for graph export
- optional helpers for snapshot hooks

### statum-inspect-core

Owns:

- protocol types
- replay engine
- machine tree model
- snapshot and diff model
- filtering and selection logic

### statum-inspect-tui

Owns:

- `ratatui` app
- panes and layouts
- keybindings
- search UX
- tree expansion and focus management

### statum-inspect-json

Owns:

- file format
- import and export helpers
- fixture generation for tests

## Protocol Model

The protocol should stay small and explicit.

Core types:

- `MachineInstanceId`
- `TransitionEvent`
- `CompositionEvent`
- `SnapshotEvent`
- `InspectorValue`

Recommended shape:

```rust
pub struct TransitionEvent {
    pub sequence: u64,
    pub machine_instance: MachineInstanceId,
    pub machine_type: &'static str,
    pub from_state: String,
    pub transition: String,
    pub to_state: String,
    pub parent_machine: Option<MachineInstanceId>,
    pub timestamp: Option<SystemTime>,
}

pub enum CompositionEvent {
    AttachChild {
        parent: MachineInstanceId,
        child: MachineInstanceId,
        role: String,
    },
    DetachChild {
        parent: MachineInstanceId,
        child: MachineInstanceId,
    },
}

pub struct SnapshotEvent {
    pub sequence: u64,
    pub machine_instance: MachineInstanceId,
    pub kind: SnapshotKind,
    pub machine_fields: Option<InspectorValue>,
    pub state_data: Option<InspectorValue>,
}

pub enum InspectorValue {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    List(Vec<InspectorValue>),
    Map(Vec<(String, InspectorValue)>),
    Opaque(String),
    Redacted,
}
```

The protocol should prefer generic structural data over stringly ad hoc blobs.

## Integration Surface

Each application should implement a small adapter surface.

Minimum adapter:

- stable machine instance ids
- transition event emission

Recommended adapter:

- snapshot projection for machine fields
- snapshot projection for state data
- composition events for parent and child machines

Nice-to-have adapter:

- redaction rules
- custom labels
- custom diff formatting
- external correlation ids

Possible traits:

```rust
pub trait InspectMachine {
    fn machine_instance_id(&self) -> MachineInstanceId;
    fn machine_fields_value(&self) -> InspectorValue;
    fn state_data_value(&self) -> Option<InspectorValue>;
}

pub trait InspectLabel {
    fn inspector_label(&self) -> Option<String> {
        None
    }
}
```

## Phases

### Phase 0: Pin the Protocol

Deliverables:

- protocol types in `statum-inspect-core`
- JSON schema or file format contract
- authority docs for static graph, runtime replay, and snapshots

Success criteria:

- no UI code decides semantics
- protocol makes missing capabilities explicit

### Phase 1: Graph Viewer MVP

Deliverables:

- load one static graph
- render graph and state list
- inspect roots and legal next transitions

Success criteria:

- graph-only mode is useful without runtime integration

### Phase 2: Replay MVP

Deliverables:

- load runtime transition events
- build per-instance timelines
- step forward and backward
- show selected transition and resulting state

Success criteria:

- exact executed path is replayable without snapshots

### Phase 3: Snapshot Support

Deliverables:

- snapshot protocol
- before and after view
- generic structural diff
- explicit unavailable state when no snapshot exists

Success criteria:

- data changes are inspectable when provided
- missing data is shown as missing, not guessed

### Phase 4: Composition Tree

Deliverables:

- parent and child machine tree
- attach and detach events
- nested timeline stepping

Success criteria:

- user can step into and out of sub-machines without losing replay context

### Phase 5: App Adapter Ergonomics

Deliverables:

- helper traits
- helper macros or derive support if justified
- redaction hooks
- custom labels

Success criteria:

- a new app can integrate without hand-rolling the whole protocol

### Phase 6: Polish

Deliverables:

- search
- filters
- saved layouts
- export helpers
- snapshot fixtures for tests

Success criteria:

- day-to-day inspector usage feels fast and predictable

## Testing Plan

Static graph tests:

- graph-only load
- exact branch targets
- multiple roots
- zero-root cycles

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

- graph-only mode with no runtime trace
- runtime trace with no snapshots
- composition events with unknown machine ids
- duplicate machine instance ids
- stale graph and runtime protocol version mismatch

## Open Questions

- should transition event emission live in `statum` itself or in a sidecar
  crate?
- should snapshot projection be trait-based, macro-based, or both?
- should the first renderer be graph-first, timeline-first, or split-view by
  default?
- how much redaction should live in app code versus inspector config?

## Checklist

- [ ] Create `statum-inspect-core`
- [ ] Define protocol types for graph replay, composition, and snapshots
- [ ] Define the JSON import and export contract
- [ ] Add graph-only inspector mode
- [ ] Add replay session model
- [ ] Add timeline stepping
- [ ] Add per-instance selection
- [ ] Add snapshot events and generic structural diffing
- [ ] Add explicit unavailable states for missing snapshot data
- [ ] Add composition events and machine tree navigation
- [ ] Add opt-in helper hooks on the Statum side
- [ ] Add one sample application adapter
- [ ] Add TUI panes and keybindings
- [ ] Add graph-only tests
- [ ] Add replay tests
- [ ] Add composition tests
- [ ] Add snapshot and diff tests
- [ ] Add adversarial protocol and mismatch tests
- [ ] Document the authority boundary in crate docs
- [ ] Decide whether redaction lives in adapters, config, or both
