# Non-Authoritative Scanning Refactor Plan

This plan exists to remove file scanning and expansion-time registries from
Statum's semantic authority path.

## Goal

Keep the public `#[state]`, `#[machine]`, `#[transition]`, and
`#[validators(...)]` model as close as possible to the current API while moving
semantic truth onto stronger observation points.

Today too much correctness depends on:

- call-site file and line lookup
- handwritten source scanning for module membership
- process-global loaded-item registries
- expansion-order discovery by name

That is acceptable for diagnostics. It is not a strong enough base for
features that claim exact or authoritative semantics.

## Authority Contract

Claimed authority surface:

- exact state-family membership
- exact machine/state linkage
- exact validator coverage for the machine's states
- exact transition target extraction for introspection
- exact runtime join from recorded transitions back to the static graph

Current observation point:

- parsed AST of the current attributed item
- raw source text and line-based module scanning
- macro-expanded items only after they have been copied into global registries
- expansion order inferred by source position and loaded-item state

Target observation point:

- parsed `#[state]` enum AST
- parsed `#[machine]` struct AST
- parsed `#[transition]` impl AST
- parsed `#[validators]` impl AST
- generated hidden helper items emitted by earlier Statum macros and resolved
  through ordinary Rust expansion

Non-goal:

- perfect semantic authority from arbitrary crate-wide source text without
  changing the macro architecture

The refactor should prefer normal generated Rust items over hidden registry
state. If a case cannot be supported from the stronger observation point, the
macro should reject it explicitly.

## Recommendation

Implement this as a generated-contract refactor, not as a scanner rewrite and
not as a stronger registry.

`#[state]` and `#[machine]` should emit hidden, deterministic support items
that later Statum macros can rely on through normal Rust resolution. Scanners
and registries can remain temporarily for diagnostics, but they should stop
deciding whether semantics are correct.

If the current split macro surface proves too restrictive for a fully
authoritative `#[validators]` design, add an opt-in grouped authority surface
as the fallback. Do not move semantic truth back into a process-global
registry.

## Rejected Direction

Do not treat the proc-macro registry as semantic authority.

Why:

- its observation point is still expansion-time discovery, not just parsed
  attributed items plus generated support items
- it is vulnerable to ordering and cross-item loading concerns that the rest of
  this refactor is trying to eliminate
- it does not solve the remaining validator problem, which is machine-owned
  builder shape rather than uniqueness or lookup

An in-process registry is acceptable only for:

- caching
- duplicate-work avoidance
- friendlier diagnostics

It is not acceptable as the source of truth for legality, state membership, or
validator rebuild shape.

## Layering

Narrative layer:

- the current user-facing attribute model

Stage layer:

- generated hidden machine and state support items that expose the exact shape
  needed by later Statum macros

Protocol-truth layer:

- parsed `#[state]` enum AST
- parsed `#[machine]` struct AST
- parsed `#[transition]` impl AST
- parsed `#[validators]` impl AST plus machine-generated helper contracts

Plain-function leaves:

- source scanners
- file-path helpers
- same-name candidate search
- friendly error hint generation

Duplication risks:

- repeating state-family structure in both `#[state]` and `#[machine]`
- repeating machine field lists and slot shape in both `#[machine]` and
  `#[validators]`

Locality risks:

- spreading one semantic decision across scanner code, registries, and macro
  modules

Invariant-placement risks:

- validating state membership from scanner-discovered metadata instead of from
  generated state-family contracts
- validating machine existence from loaded registries instead of generated
  machine contracts
- generating validator builders from validator-local reconstruction instead of
  from machine-owned shape

## Design Bet

The strongest path that still preserves most of the current API is:

1. `#[state]` emits authoritative state-family support items.
2. `#[machine]` consumes those support items instead of rediscovering the state
   enum through registries.
3. `#[machine]` emits authoritative machine support items.
4. `#[transition]` consumes those machine support items instead of looking up
   loaded machine metadata.
5. `#[validators]` consumes machine support items for validator coverage and
   payload legality.
6. `#[machine]` owns validator rebuild structure, and `#[validators]` only
   plugs semantics into that structure.

This last point is the course correction for the validator phase. The remaining
problem is validator-side reconstruction still trying to own machine builder
shape.

## Validator Course Correction

The clean design is:

- `#[state]` remains authoritative for the state family
- `#[machine]` remains authoritative for machine fields, builder slots, setter
  generation, and rebuild output shape
- `#[validators]` becomes authoritative only for validator method coverage,
  return-shape legality, and payload/state pairing

That means `#[machine]` should emit a hidden validator-support macro that owns:

- machine field list and field types
- stable field-slot identity
- builder types and setter generation
- `into_machine()` / `.into_machines()` rebuild shape
- output paths such as `machine::SomeState`, `machine::Fields`, and related
  helper traits

Then `#[validators]` should:

- parse validator methods from the impl AST
- verify one method per machine state variant
- verify payload type compatibility and supported return wrappers
- pass the generated validator checks into the machine-emitted support macro

In other words:

- `#[machine]` provides structure
- `#[validators]` provides semantics

This keeps the last hard authority boundary aligned with the item that actually
owns the shape.

## Phases

### Phase 0: Pin the Contract

Before changing code, document the rule for every authority claim:

- what surface is claimed to be exact
- what observation point is allowed to justify that claim
- what unsupported cases must be rejected

Deliverables:

- narrow any docs that currently imply stronger authority than the mechanism
- add comments near the macro entry points describing the new contract

### Phase 1: State-Family Authority Surface

Teach `#[state]` to emit the full hidden state-family contract needed by later
macros.

Add or formalize:

- a sealed state-family trait
- a per-marker trait with associated family, state id, and data type
- deterministic helper items for later macro expansion
- a visitor-style helper surface for enumerating every state variant without
  source scanning

The visitor surface is the main escape hatch for `#[machine]`: it needs the
full variant list to generate `SomeState`, builders, and related support, and
it cannot recover that list authoritatively from separate item-local AST alone.

Success criteria:

- `#[state]` remains the only place that parses the enum variants
- later macros no longer need loaded `EnumInfo` to recover the variant list

### Phase 2: Machine Refactor Off Loaded State Lookup

Refactor `#[machine]` so semantic generation depends on:

- the machine struct AST
- the first generic parameter or explicit state-family link
- the generated state-family support items from Phase 1

Remove loaded-state lookup from the generation path in:

- machine metadata resolution
- machine validation that only exists to discover the state enum shape

Keep or rework:

- cfg rejection on machine fields
- hidden support module generation
- builder generation
- introspection id surfaces

Decision point:

- either keep the current first-generic convention
- or introduce an explicit link such as `#[machine(state = TaskState)]`

The explicit link is a valid trade if it materially reduces hidden coupling.

Success criteria:

- `#[machine]` no longer needs loaded state registries for variant structure
- scanners and registries, if still present, only improve diagnostics

### Phase 3: Transition Refactor Off Loaded Machine Lookup

Refactor `#[transition]` so semantic validation depends on:

- the parsed impl target
- parsed method return types
- machine-generated support items
- state-family marker contracts

Replace loaded-machine lookup with deterministic machine support items emitted
by `#[machine]`.

Keep the current fail-closed rule for unsupported return wrappers. The current
restriction is good and should stay explicit.

Add trait-level or generated-item checks for:

- impl target belongs to a Statum machine family
- source marker belongs to that family
- every returned marker belongs to that family

Success criteria:

- `#[transition]` does not consult loaded-machine registries to decide whether
  the impl is valid
- transition graph emission is driven by the impl AST plus generated machine
  contracts

### Phase 4: Validators Semantics Off Loaded State Lookup

Split the validator work into semantics first and structure second.

In this phase, `#[validators]` should stop depending on loaded state metadata
for:

- validator coverage
- state membership
- payload/state pairing

The machine validator contract must expose enough state-family truth for
`#[validators]` to verify:

- one validator method per state variant
- only valid validator method names
- correct payload type for data-bearing states
- supported return wrapper shapes

Success criteria:

- validator coverage and payload matching no longer rely on loaded state
  registries
- unsupported validator method shapes fail closed

### Phase 4b: Move Validator Builder Shape Under `#[machine]`

This is the actual fix for the remaining validator gap.

Do not keep generating validator builder shape in `#[validators]` from field
lists and local slot booleans. That duplicates machine structure in the wrong
place.

Instead, `#[machine]` should emit a hidden validator-support macro that owns:

- field-slot identity
- builder type parameters and completeness tracking
- setter methods
- single-item rebuild shape
- batch rebuild shape

`#[validators]` should invoke that support macro with:

- generated per-variant validator checks
- generated per-variant rebuild-report checks
- async or sync mode

Success criteria:

- validator rebuild structure no longer relies on loaded machine registries
- validator rebuild structure is no longer reconstructed independently inside
  `#[validators]`
- the remaining authority boundary for validators is parsed impl AST plus
  machine-emitted support items

### Phase 5: Narrow or Delete the Scanner Path

Once Phases 1 through 4b land:

- remove scanner and registry code from semantic validation
- keep only the pieces that improve diagnostics
- or delete them if the maintenance cost stays high

At this point:

- `module_path_extractor`
- `macro_registry`
- loaded machine/state registries

should either be diagnostics-only or gone.

## Fallback Plan

If Phase 4b proves too costly while preserving the current split-attribute API,
introduce an opt-in grouped authority mode.

Candidate shapes:

- `#[statum] mod workflow { ... }`
- `statum::define! { ... }`

That grouped surface would let one macro observe the full state, machine,
transitions, and validators together without cross-item discovery tricks.

This is preferable to keeping scanner-driven semantics for the hardest surface,
and preferable to restoring a semantic registry.

## Adversarial Test Matrix

Every phase should extend tests for:

- `#[cfg]` on whole impl blocks
- `#[cfg]` on methods inside impl blocks
- `macro_rules!`-generated items
- `include!`-generated items
- nested modules
- same-name items in sibling modules
- duplicate-id pressure for transition and state ids
- stale cache behavior, if any diagnostics cache remains

Additional phase-specific tests:

- Phase 1: visitor surface covers unit, tuple, and named-field states
- Phase 2: machine generation succeeds without any loaded-state registry data
- Phase 3: transition validation succeeds without any loaded-machine registry
  data
- Phase 4: validators semantic checks succeed without any loaded state registry
  data
- Phase 4b: validators rebuild generation succeeds without any loaded machine
  registry data and without validator-local builder reconstruction

## Suggested Order of Code Changes

1. Add new hidden state-family support items under `#[state]`.
2. Teach `#[machine]` to consume that support while leaving old lookups in
   place behind assertions or temporary fallback.
3. Delete the machine-side semantic dependency on loaded state lookup.
4. Add machine-generated support items for transitions and validator semantics.
5. Teach `#[transition]` to use the new machine support items.
6. Teach `#[validators]` to use the machine validator contract for state
   coverage and payload legality.
7. Add a machine-owned validator-support macro for rebuild structure.
8. Teach `#[validators]` to plug validator checks into that machine-owned
   structure instead of generating builders itself.
9. Delete or quarantine scanner and registry semantics.
10. Update docs to state the new authority contract and unsupported cases.

## Merge Gate

Do not call this complete until all of the following are true:

1. Claimed authority surface:
   transition legality, machine/state linkage, validators coverage, validator
   rebuild shape, and introspection structure are generated from item-local AST
   plus generated support items, not source scanning.
2. Actual observation point:
   parsed attributed items and generated helper items resolved through normal
   Rust expansion.
3. Unsupported cases:
   explicitly rejected rather than guessed.
4. Registry role:
   limited to diagnostics or caching, not legality or rebuild shape.
5. Adversarial tests:
   present for cfg, macro-generated items, include-generated items, nested
   modules, sibling same-name items, and duplicate-id pressure.

Until then, compatibility is improving, but authority is not fully fixed.
