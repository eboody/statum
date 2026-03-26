# Statum Exact Static Relation Plan

This plan covers the exact static substrate that file export, graph renderers,
and the future inspector TUI will consume.

## Goal

Developers should annotate as little as possible.

Statum should:

- infer relations from visible types and signatures first
- require declarations only when the relation is invisible at the use site
- attach any required declaration to the reference type once, not at every
  field or method

## Scope

This plan is exact-only.

It covers:

- linked compiled machine topology
- declared validator-entry surfaces
- direct construction availability per state
- exact cross-machine relations from supported static observation points

It does not cover:

- transition-body analysis
- orchestration hidden in helper functions
- runtime event flow
- primitive ids with no typed wrapper

## Current Status

Completed:

- canonical `CodebaseRelation` export surface
- legacy `links()` compatibility for payload-only links
- validator-entry overlays in codebase export
- direct-construction availability exported as state metadata
- exact relation extraction from state payloads, machine fields, and transition
  parameters
- trait-backed nominal `#[machine_ref(...)]`
- fail-closed authority guards for unsupported validator cfg shapes
- fail-closed authority guards for same-name wrapper lookalikes and ambiguous
  direct machine syntax

Still open:

- visible builder markers in Mermaid, DOT, and PlantUML
- relation-derived machine summary edges in graph renderers
- richer relation-provenance helpers for downstream consumers

## Authority Contract

Claimed authority surface:

- exact static topology from linked compiled Statum machine metadata
- exact declared validator-entry surfaces from compiled `#[validators]` impls
- exact direct-construction availability per exported state
- exact relations from supported direct type syntax plus trait-backed nominal
  `#[machine_ref(...)]` declarations

Actual observation point:

- `MachineIntrospection::GRAPH`
- linked compiled machine, validator-entry, relation, and reference-type
  inventories
- macro-expanded, cfg-pruned `#[machine]`, `#[transition]`, `#[validators]`,
  and `#[machine_ref]` items

Supported exact inference:

- state payload relations
- machine field relations
- transition parameter relations
- validator entry surfaces
- direct-construction availability
- nominal opaque reference types declared with `#[machine_ref(...)]`
- canonical absolute transparent carriers such as
  `::core::option::Option<...>`, `::core::result::Result<..., E>`,
  `::alloc::vec::Vec<...>`, `::alloc::boxed::Box<...>`,
  `::alloc::rc::Rc<...>`, and `::alloc::sync::Arc<...>`
- explicit direct machine targets written with `crate::`, `self::`, `super::`,
  or absolute paths

Rejected or ignored in the exact lane:

- imported wrapper aliases
- bare prelude wrapper names
- unanchored direct machine paths
- generic state arguments in direct machine syntax
- transition-body calls and helper-function orchestration
- primitive ids with no nominal wrapper

Fail-closed rule:

Unsupported exact cases must reject explicitly or contribute no exact relation.
They must never be exported as best-effort exact metadata.

## Layering

Narrative layer:

- codebase graph export and inspector-facing relation navigation

Stage layer:

- `CodebaseDoc`
- future workspace session model in the inspector

Protocol-truth layer:

- linked compiled machine inventory
- linked validator-entry inventory
- linked exact relation inventory
- linked nominal reference-type inventory

Plain-function leaves:

- renderer formatting
- bundle writing
- label formatting
- relation sorting and filtering

Duplication risks:

- keeping relation semantics in both `links()` and `relations()`
- teaching renderers relation rules instead of projecting one canonical model

Locality risks:

- pushing provenance interpretation into the TUI instead of keeping it in the
  export model
- letting graph renderers invent relation meaning that JSON does not carry

Invariant-placement risks:

- relaxing direct-type rules in the renderer layer
- reintroducing suffix matching for exact relation targets
- inferring runtime meaning from exact static relations

## Canonical Export Model

The exact substrate should keep one canonical relation record with:

- stable relation index
- relation kind
- relation basis
- exact source ref
- exact target machine and state
- optional declared reference type path when the basis is nominal

Relation kinds:

- `state_payload`
- `machine_field`
- `transition_param`

Relation basis:

- `direct_type_syntax`
- `declared_reference_type`

Source refs:

- `StatePayload { machine, state, field_name }`
- `MachineField { machine, field_name, field_index }`
- `TransitionParam { machine, transition, param_index, param_name }`

Builder availability should stay as a state property:

- `direct_construction_available: bool`

It should not become a synthetic entry node.

## Renderer Plan

JSON stays the richest exact surface.

Mermaid, DOT, and PlantUML should project exact metadata without inventing
new semantics.

Planned renderer additions:

- compact builder markers on states that expose direct construction
- machine summary edges derived from exact relations
- relation-style distinctions that preserve provenance class at a glance

Defaults:

- builder availability is a compact state marker, not an entry node
- machine summary edges are derived from `relations()`, not a second authority
  inventory
- legacy `links()` remain compatibility output and should not drive new TUI
  features

## Phases

### Phase 0: Lock The Exact Substrate

Deliverables:

- canonical `CodebaseRelation` surface
- trait-backed `#[machine_ref(...)]`
- fail-closed authority docs

Status:

- done

### Phase 1: Builder Overlay Rendering

Deliverables:

- visible builder markers in Mermaid
- visible builder markers in DOT
- visible builder markers in PlantUML
- stable JSON coverage remains unchanged

Success criteria:

- users can see direct-construction availability in graph outputs without
  mistaking it for initial-state semantics

### Phase 2: Relation Projection And Summary Edges

Deliverables:

- machine summary edges derived from exact relations
- stable relation-grouping helpers for downstream consumers
- renderer conventions for relation provenance classes

Success criteria:

- graph outputs make cross-machine static coupling legible without hiding the
  underlying exact relation records

### Phase 3: Inspector-Facing Provenance Helpers

Deliverables:

- helpers for inbound and outbound relation lookup
- stable detail payloads for relation provenance
- relation-group filtering surfaces

Success criteria:

- the TUI can answer what points at this and why without reconstructing export
  semantics itself

## Checklist

- [x] Export exact relations from state payloads
- [x] Export exact relations from machine fields
- [x] Export exact relations from transition parameters
- [x] Export validator-entry overlays
- [x] Export direct-construction availability per state
- [x] Support trait-backed nominal `#[machine_ref(...)]`
- [x] Reject unsupported exact validator cfg shapes
- [x] Reject same-name wrapper lookalikes in the exact lane
- [x] Reject unanchored direct machine syntax in the exact lane
- [x] Keep `links()` as compatibility output while making `relations()` the
      canonical exact surface
- [ ] Render builder availability visibly in Mermaid
- [ ] Render builder availability visibly in DOT
- [ ] Render builder availability visibly in PlantUML
- [ ] Add machine summary edges derived from `relations()`
- [ ] Add relation lookup helpers for inbound and outbound navigation
- [ ] Add stable provenance detail helpers for the inspector

## Acceptance Criteria

This plan is complete when:

- developers get payload, field, transition-parameter, validator, and builder
  exact surfaces with zero per-use annotations when the type surface is explicit
- opaque references need one declaration on the nominal wrapper type, not on
  each use
- every exported exact relation can explain why it exists through kind and
  basis
- graph outputs show builder availability and machine-level coupling without
  weakening the exact JSON surface
