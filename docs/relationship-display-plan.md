# Exact Relationship Display Plan

This file tracks the next exact-static milestone after the heuristic inspector
lane shipped: make the graph bundle, `cargo statum-graph` CLI, and inspector
TUI all show real cross-machine relationships from downstream codebases.

## Summary

Goal:

- exact relationships should render in `codebase.mmd`, `codebase.dot`,
  `codebase.puml`, and `codebase.json`
- `cargo statum-graph codebase` should succeed on downstreams that use the
  supported exact relation surfaces
- `cargo statum-graph inspect` should surface those same exact relationships
  in the exact lane, with heuristics filling only the remaining gaps

Current blocker:

- the original multi-producer `#[via(...)]` blocker is fixed
- Citacell exact export now succeeds and surfaces 4 exact relations / 2 exact
  machine-summary edges:
  - broker -> outbound_release
  - broker -> result_intake
- the remaining downstream gap is exact promotion of nominal artifact and
  handoff types such as Citacell's write-back / correlation handoff path,
  which still needs `#[machine_ref(...)]` adoption to move from heuristic-only
  visibility into the exact lane

## Authority Contract

Exact relationship display remains authoritative only when it comes from:

- `MachineIntrospection::GRAPH`
- linked compiled machine inventories
- linked compiled validator-entry inventories
- linked compiled exact relation inventories
- linked compiled attested-route inventories
- linked compiled nominal `#[machine_ref(...)]` declarations

Heuristic discovery stays:

- TUI-only
- non-authoritative
- separate from exported graph files and `CodebaseDoc`

Malformed or ambiguous exact relation inventories must still fail closed. The
goal is not to weaken exactness. The goal is to align the exact codebase layer
with the exact relation surfaces Statum already allows upstream.

## Recommended Direction

Recommended direction:

- treat one attested route as able to map to multiple compatible producer
  transitions when the macro layer already accepts that route reuse
- represent that multiplicity explicitly in the exact codebase model and
  relationship detail surfaces instead of rejecting it as malformed

Rejected direction unless design pressure forces it:

- keep one-producer-only attested routes in `CodebaseDoc`, but then move
  duplicate-route rejection to the macro layer so downstreams fail at the
  declaration site rather than at graph-export time

The first direction matches current downstream usage better and preserves the
value of the exact relationship lane for real codebases such as Citacell.

## Phase 1: Align Attested Route Identity

Deliverables:

- settle the exact attested-route identity contract across macros, linked
  inventory, `CodebaseDoc`, CLI, and inspector detail
- remove the current mismatch where compatible route reuse compiles but exact
  codebase export rejects it later
- make `CodebaseDoc::linked()` represent or resolve multi-producer attested
  routes deterministically

Implementation notes:

- if one attested route can have multiple producer transitions, replace the
  singular resolved-route model with a grouped producer set
- keep grouped producer ordering stable
- keep exact relation detail precise:
  - one producer stays one producer in the UI
  - multiple producers must show a stable producer list, not a guessed
    collapse
- if some multi-producer shapes are still unsupported, reject those explicitly
  with a truthful error instead of silently dropping them

Success criteria:

- Citacell no longer fails with `DuplicateViaRoute`
- exact codebase export reaches rendering for downstreams using compatible
  multi-producer `#[via(...)]` routes

Status:

- done
- attested producer routes now join consumers by compiler-resolved route
  marker type identity
- compatible multi-producer routes are grouped deterministically in
  `CodebaseDoc`

## Phase 2: Project Exact Relationships Across All Surfaces

Deliverables:

- graph backends show exact cross-machine relationship edges derived from the
  canonical exact relation surface
- CLI bundle output and inspector exact lane show the same relationship set
- exact relation detail stays richer than summary edges, especially for
  attested routes

Implementation notes:

- keep `CodebaseDoc` as the one canonical exact relationship model
- keep Mermaid, DOT, PlantUML, and JSON as projections of that one model
- keep inspector exact relation lists and summary groups driven by
  `CodebaseDoc`, not by re-derived logic
- make attested-route detail readable when one route has multiple producers

Success criteria:

- the graph files, CLI export, and TUI exact lane all agree on which exact
  machine-to-machine relationships exist
- exact relation detail explains why a relationship exists without requiring
  heuristic mode

Status:

- partly done
- graph bundle, CLI export, and inspector exact lane now agree on Citacell's
  broker -> outbound_release and broker -> result_intake relationships
- remaining exact downstream coverage depends on downstream
  `#[machine_ref(...)]` adoption for nominal artifact and handoff types

## Phase 3: Downstream Validation

Primary validation target:

- Citacell

Required validation:

- `cargo statum-graph codebase /home/eran/code/citacell` succeeds
- the output bundle is written successfully
- `cargo statum-graph inspect /home/eran/code/citacell` opens and shows exact
  relationships in the exact lane
- exact relations are greater than zero
- at minimum, these relationships appear in the exact lane:
  - broker -> outbound_release
  - broker -> result_intake
  - write_back -> correlation

Secondary validation:

- a workspace with malformed exact relation inventory still fails closed with
  a specific error
- heuristic mode still works and still does not leak into the export bundle

## Test Plan

Add or update tests for:

- compatible multi-producer `#[via(...)]` routes on one machine
- incompatible multi-producer `#[via(...)]` routes that must still reject
- stable producer ordering for grouped attested-route detail
- exact relation rendering snapshots for attested-route-backed relationships
- CLI end-to-end export when attested routes are present
- inspector exact-lane detail when one attested route has multiple producers
- mixed mode behavior when heuristic and exact relationships overlap

## Checklist

- [x] Decide and document the exact attested-route identity contract
- [x] Align macro, linked inventory, and `CodebaseDoc` behavior with that
      contract
- [x] Remove the current `DuplicateViaRoute` blocker for supported downstreams
- [ ] Keep unsupported attested-route shapes fail-closed
- [x] Ensure graph backends project the canonical exact relationship set
- [x] Ensure `cargo statum-graph codebase` and `inspect` consume the same exact
      relationship substrate
- [x] Add or update exact relation detail for multi-producer attested routes
- [ ] Validate the end-to-end flow against Citacell
- [ ] Keep heuristic discovery separate and unchanged in exported files
- [ ] Update README and introspection docs once the contract is implemented

## Acceptance Criteria

This roadmap is complete when:

- a real downstream that uses `#[via(...)]` and `#[machine_ref(...)]` can
  render exact relationships through the graph bundle, CLI, and TUI
- the exact lane remains authoritative and fail-closed
- heuristic mode is no longer required just to see the important
  cross-machine couplings in that downstream
