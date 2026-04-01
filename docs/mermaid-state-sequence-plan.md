# Mermaid State And Sequence Diagram Plan

## Summary

Short answer:

- Yes, Statum already has enough exact structure for machine-local
  `stateDiagram-v2`.
- Yes, Statum already has enough exact structure for composition-machine
  `stateDiagram-v2` when that means "the composition machine's own states and
  transitions".
- Partly, Statum has enough exact structure for composition-machine diagrams
  that reference protocol machines.
- Partly, Statum has enough exact structure for `sequenceDiagram`.

The important boundary is this:

- exact protocol legality already exists
- exact composition relations already exist
- exact detached handoff provenance already exists
- exact workspace path preference already exists
- exact runtime chronology does not exist
- exact nested submachine hierarchy does not exist as a first-class exported
  model

So v1 should generate:

- exact machine-local state diagrams
- exact composition-machine state diagrams
- exact composition annotations on those diagrams
- exact relation-centric and path-centric sequence diagrams only where the
  ordering is truly derivable

V1 should not claim:

- exact end-to-end runtime traces
- exact full hierarchical embedding of one protocol machine inside another
- exact cross-machine event order when the surface only proves connectivity

## Authority Boundary

Claimed authority surface:

- `stateDiagram-v2` for one machine
- composition-machine state diagrams as outer workflow truth
- exact sequence diagrams for exact handoff and exact composition-transition
  paths

Actual observation points:

- machine-local structure:
  [`MachineIntrospection::GRAPH`](/home/eran/code/statum/statum-graph/src/lib.rs#L1)
- stable renderer model:
  [`ExportDoc`](/home/eran/code/statum/statum-graph/src/export.rs#L8)
- linked workspace structure and exact cross-machine relations:
  [`CodebaseDoc`](/home/eran/code/statum/statum-graph/src/lib.rs#L7)
- exact composition ownership:
  [`#[machine(role = composition)]`](/home/eran/code/statum/statum/src/lib.rs#L343)
- exact attested handoff provenance:
  [introspection.md](/home/eran/code/statum/docs/introspection.md#L190)

What is exact now:

- machine identity
- state identity
- transition-site identity
- legal targets
- graph roots
- machine role
- exact relation source kind
- exact target machine and target state
- exact attested producer machine, state, and transition
- exact grouped composition-owned summaries

What is not exact now:

- exact source code locations for exact path steps
- exact runtime execution order across multiple machines
- exact hierarchical containment of full child protocol graphs inside
  composition states
- exact chosen producer when one attested route legally matches multiple
  producers

Unsupported-case policy:

- do not approximate missing order
- do not flatten child protocol graphs into one outer state chart unless the
  narrowed claim says it is a projection
- when a sequence step cannot justify an arrow, emit a note or omit it
- when one exact relation has multiple legal producers, render explicit
  alternatives or require a selector; do not silently choose one

## Current Truth Surfaces

### Machine Truth

`MachineDoc` is already authoritative for one machine's local topology:

- exact states
- exact transition sites
- exact legal targets
- graph roots derivable from the static graph

Relevant code:

- [`MachineDoc`](/home/eran/code/statum/statum-graph/src/lib.rs#L56)
- [`ExportDoc`](/home/eran/code/statum/statum-graph/src/export.rs#L8)

This is enough to generate an exact `stateDiagram-v2` for one machine.

It is also enough to derive:

- Mermaid start edges from roots
- Mermaid end edges from sink states with no outgoing transitions
- labels from `#[present(...)]` data when available
- short descriptions from `MachinePresentation`

### Composition Truth

Composition machines already export exact composition-owned relations whenever
they directly carry child machines or exact detached handoff evidence:

- state payloads
- machine fields
- transition parameters
- `#[via(...)]` consumers
- raw `statum::Attested<_, Route>` wrappers

Relevant code and docs:

- [`#[machine(role = composition)]`](/home/eran/code/statum/statum/src/lib.rs#L343)
- [composition roadmap phase 2 and 3](/home/eran/code/statum/docs/composition-machine-roadmap.md#L114)
- [`CodebaseRelation`](/home/eran/code/statum/statum-graph/src/codebase/mod.rs#L608)
- [`CodebaseRelationSource`](/home/eran/code/statum/statum-graph/src/codebase/mod.rs#L559)
- [`CodebaseAttestedRoute`](/home/eran/code/statum/statum-graph/src/codebase/mod.rs#L677)

This is enough to generate exact composition annotations and exact
machine-to-machine handoff diagrams.

### Path Truth

The current inspector path explorer prefers:

- composition-owned exact paths
- then raw exact paths
- then heuristic fallback

Relevant code:

- [roadmap phase 4](/home/eran/code/statum/docs/composition-machine-roadmap.md#L170)
- [path item discovery](/home/eran/code/statum/cargo-statum-graph/src/inspect.rs#L1451)
- [path edge building](/home/eran/code/statum/cargo-statum-graph/src/inspect.rs#L1518)

Important limitation:

- current path discovery is BFS over relation summaries
- that is exact graph reachability
- that is not exact event chronology

So path data can seed sequence export only when the sequence semantics are tied
back to ordered composition transitions, not when they come only from BFS over
machine relation groups.

## Design Goals

### State Diagram Goals

- make protocol legality visible in the native Mermaid shape
- make composition machines readable as first-class workflow truth
- annotate exact child-machine and attested-handoff evidence without weakening
  the claim
- keep the generated Mermaid stable for snapshots and docs

### Sequence Diagram Goals

- show exact handoff between machines
- show exact consumer transitions on composition machines
- show producer provenance when attested routes provide it
- keep structural containment out of arrow semantics unless the source truly
  implies a message-like handoff

## Recommended Layering

Narrative layer:

- inspector tabs
- preview mode
- "open source" and "preview with termaid"

Stage layer:

- diagram selection
- path selection
- diagram planning
- exactness labeling

Protocol-truth layer:

- `MachineDoc`
- `ExportDoc`
- `CodebaseDoc`
- exact relation records
- exact attested route records

Plain-function leaves:

- Mermaid text formatting
- node id escaping
- path naming
- file emission
- `termaid` invocation

Do not put semantic inference in the renderer or preview layer.

## State Diagram Plan

### V1 Output Types

V1 should support three exact state-diagram products.

1. Protocol machine state diagram
2. Composition machine state diagram
3. Composition machine state diagram with exact relation annotations

The third item is still the composition machine's own graph. It is not a claim
that the entire child machine graph is nested inside the outer machine.

### Protocol Machine State Diagram

Input:

- `MachineDoc`
- or `ExportDoc`

Mapping:

- header: `stateDiagram-v2`
- each state becomes one Mermaid state node
- each transition site becomes one or more Mermaid arrows, one per legal target
- each root becomes `[*] --> State`
- each sink state becomes `State --> [*]`
- labels come from `display_label()`
- transition labels come from `display_label()`

Nice-to-have enrichment:

- machine label as top comment
- state descriptions as notes
- transition descriptions as notes

Exactness status:

- exact

### Composition Machine State Diagram

Input:

- `CodebaseMachine` for a machine with composition role

Mapping:

- same core mapping as protocol machine state diagram
- outer states and outer transitions are the only state-machine truth
- machine role may appear in a title comment or note

Exactness status:

- exact

### Composition Annotation Rules

Exact composition annotations should attach to the outer machine graph by
relation source kind.

For `StatePayload` relations:

- attach the child machine target state to the outer state
- recommended rendering:
  note on the outer state or state-label suffix
- exact meaning:
  "this outer state carries a child machine already at this exact child state"

For `MachineField` relations:

- attach machine-level notes, not per-state nesting
- exact meaning:
  "this composition machine struct carries this child-machine relation"

For `TransitionParam` direct relations:

- attach annotations to the outer transition
- exact meaning:
  "this transition consumes a machine value in this exact target state"

For attested handoff relations:

- attach route name and producer summary to the outer transition
- exact meaning:
  "this transition consumes an exact attested handoff that can originate from
  these producer transitions"

Relevant model:

- [`CodebaseRelationSource`](/home/eran/code/statum/statum-graph/src/codebase/mod.rs#L559)
- [`CodebaseRelationSemantic`](/home/eran/code/statum/statum-graph/src/codebase/mod.rs#L540)

### What Not To Do In Exact State Diagrams

Do not, by default:

- inline the full child protocol graph under an outer composition state
- connect outer transitions directly to inner child transitions
- imply that a machine field relation is a substate
- imply that a transition-param relation proves a producer transition when it
  is not attested

Why:

- a composition state often carries a child machine at one specific child
  state, not the whole child-machine protocol
- a machine field is structural, not a substate
- a transition parameter may prove a required child state, but not the prior
  producer transition

Example:

- `Reviewing(review::Machine<Pending>)` proves the child machine is in
  `Pending`
- it does not prove that the full `review::Machine` protocol is nested under
  `Reviewing`

### Optional Projected View

After the exact lane ships, a second view may be added:

- `stateDiagram-v2` projected atlas view

This projected view may:

- inline child protocol diagrams
- cluster related child states
- create visual hierarchy

But it must be labeled as a projection, not exact protocol truth.

### Aspirational Exact Child-Slot Hierarchy

Longer term, Statum can support a stronger exact hierarchy surface, but that
requires new exported truth, not just a different renderer.

Needed additions:

- stable child-slot identity on composition machines
- per-state child-slot occupancy
- per-transition child-slot continuity semantics
- fail-closed ambiguity rules when slot continuity cannot be proven

That future surface could support an exact nested hierarchy claim, but only if
the claim is narrowed to "exact child-slot occupancy and continuity" rather
than "the whole child protocol graph is always a true Mermaid substate of the
outer machine."

Until that surface exists, full nested child-protocol hierarchy should stay in
the aspirational lane.

## Sequence Diagram Plan

### Sequence Exactness Rule

A sequence diagram is exact only when each arrow is backed by one exact
ordered handoff step.

That means:

- the exporter must know the ordered consumer transition path
- each arrow must map to one exact relation or one exact attested producer
- structural references that do not imply an event should become notes or be
  omitted

### V1 Output Types

V1 should support two exact sequence products.

1. Relation-centric handoff sequence
2. Composition-path sequence

### Relation-Centric Handoff Sequence

Input:

- one `CodebaseRelationDetail`

Purpose:

- inspect one exact cross-machine handoff in sequence form

Participants:

- source machine
- target machine
- optionally producer machine when attested provenance exists and differs from
  the consumer machine

Arrow rules:

- direct `TransitionParam` relation without attestation:
  render one handoff arrow labeled with the required target state
- attested relation with one producer:
  render producer transition first, then consumer handoff
- attested relation with multiple producers:
  render Mermaid `alt` branches, one branch per legal producer

Exactness status:

- exact

### Composition-Path Sequence

Input:

- one composition machine
- one selected root-to-sink transition path through that machine

Purpose:

- explain one exact workspace handoff story in time order

Critical rule:

- order comes from the composition machine's own ordered transition path
- not from BFS over workspace relation groups

Step derivation:

1. enumerate root-to-sink transition paths in the composition machine
2. choose one path or export one file per path
3. for each composition transition on that path, inspect exact relations whose
   source is that transition parameter
4. convert those relations into sequence events
5. keep structural state-payload and machine-field relations as notes, not
   arrows

Exactness status:

- exact for the selected composition path

### Why Path Selection Is Required

Whole-machine sequence export is not a good default when the machine branches.

Problems:

- branch explosion
- unreadable diagrams
- ambiguous "one timeline" story when the machine has multiple legal paths

So the exporter should either:

- require one selected path
- or emit one sequence file per root-to-sink path

### Arrow Semantics By Source Kind

`TransitionParam`

- candidate for arrow semantics
- consumer transition exists
- event-like handoff exists

`StatePayload`

- not an event by itself
- use participant note, activation, or omit

`MachineField`

- structural relation
- use participant note or machine-level note

This distinction is required to keep sequence diagrams honest.

### Attested Route Rules

When `attested_via` exists:

- show route name in the message label
- include producer machine, producer state, and producer transition
- if exactly one producer matched, show one ordered producer event
- if multiple producers matched, render explicit alternatives

Relevant model:

- [`CodebaseAttestedRoute`](/home/eran/code/statum/statum-graph/src/codebase/mod.rs#L677)

### Direct Child Rules Without Attestation

When a composition transition consumes a direct child machine state without
attested provenance:

- the child-state handoff is exact
- the producing child transition is not exact

So the exporter may render:

- one arrow labeled as state handoff
- one note saying producer transition is not available in the current exact
  surface

It must not invent a producer transition.

## API Plan

### New Truth-Adjacent Planning Types

Add non-public planning types first.

- `MachineStateDiagramPlan`
- `CodebaseStateDiagramPlan`
- `RelationSequenceDiagramPlan`
- `CompositionPathSequenceDiagramPlan`

These are stage-layer types:

- they consume exact truth surfaces
- they shape one diagram plan
- they do not own new semantics

### Renderer Entry Points

Recommended additions:

- `statum_graph::render::mermaid_state(&ExportDoc) -> String`
- `statum_graph::codebase::render::mermaid_machine_state(&CodebaseDoc, machine_index) -> Result<String, ...>`
- `statum_graph::codebase::render::mermaid_relation_sequence(&CodebaseDoc, relation_index) -> Result<String, ...>`
- `statum_graph::codebase::render::mermaid_composition_sequence(&CodebaseDoc, machine_index, path_selector) -> Result<String, ...>`

Keep the current flowchart renderer unchanged.

### Selector Types

Add explicit selector inputs instead of hidden heuristics.

- `MachineSelector`
- `RelationSelector`
- `CompositionPathSelector`

`CompositionPathSelector` should support:

- exact path id
- root state + sink state + ordinal
- `all_paths`

## CLI Plan

### Export Surface

Keep the current workspace export bundle stable.

Add optional extra outputs rather than changing `codebase.mmd`, and do not
make those extra files mandatory for the inspector.

Recommended output layout:

- `codebase.mmd` for current flowchart overview
- `diagrams/index.json`
- `diagrams/workspace/<scope>.flow.mmd`
- `diagrams/machines/<machine-key>.state.mmd`
- `diagrams/relations/<relation-key>.sequence.mmd`
- `diagrams/composition/<machine-key>/<path-key>.sequence.mmd`

### Commands

Keep these commands stable:

- `cargo statum-graph inspect <workspace>`
- `cargo statum-graph state-diagram <workspace> --machine <type-path>`
- `cargo statum-graph sequence-diagram <workspace> --relation <index>`

Recommended additive CLI growth:

- `--include-state-diagrams`
- `--include-sequence-diagrams`
- `--include-workspace-diagrams`

Possible later namespace alias:

- `cargo statum-graph diagram workspace`
- `cargo statum-graph diagram state`
- `cargo statum-graph diagram sequence relation`
- `cargo statum-graph diagram sequence composition`

That namespace should stay additive. It should not replace the focused commands
until the ergonomics are proven better in practice.

## Inspector Plan

The next inspector should be diagram-first, not tab-first.

### Primary Modes

Recommended primary inspector modes:

- `Workspace`
- `Machine`
- `Handoff`

The current `Diagram` detail tab is not the target shape. The target shape is:

- left pane: outline, search, filters, diagnostics
- center pane: diagram viewport
- right pane: supporting semantic detail

### Workspace Diagram

The home surface should render the exact workspace flowchart in the center
pane.

Rules:

- if composition machines exist, bias initial ranking and selection toward
  them
- exact workspace flow stays exact flowchart truth
- diagnostics and heuristics stay secondary, not the primary home surface

### Machine Diagram

Machine drilldown should render exact `stateDiagram-v2` in the center pane.

Supporting surfaces stay available for:

- state detail
- transition detail
- validators
- raw Mermaid
- docs
- source
- deterministic explanation

### Handoff Diagram

Entry points:

- from exact relation detail
- from exact composition path detail

The primary diagram is:

- exact `sequenceDiagram`

If exact composition-path ordering is not yet exported as a first-class truth
surface, the inspector must keep that path unavailable rather than guessing.

### Preview Policy

The inspector should:

- generate Mermaid in memory
- preview through `termaid`
- keep raw Mermaid source available
- separate semantic selection from viewport scroll state

If `termaid` cannot render the generated Mermaid subset:

- show the Mermaid source
- show a precise unsupported-preview message
- do not down-convert to a weaker approximate diagram silently

## `termaid` Integration Plan

### Separation Of Responsibilities

Statum should:

- generate Mermaid source from exact truth

`termaid` should:

- render Mermaid source in terminal form

The inspector should:

- preview or open generated Mermaid

### Practical Near-Term Policy

Today:

- Rust `termaid` handles flowcharts and an initial state subset
- Rust `termaid` does not yet handle composite states in the path you care
  about most
- Python `termaid` already has richer state and sequence parsing

So near term:

- generate full Mermaid source in Statum
- preview through Rust `termaid` directly from in-memory Mermaid when
  supported
- allow optional reuse of diagram-bundle outputs later, but do not require
  files for preview
- otherwise fall back to Mermaid source display

## Testing Plan

### State Diagram Tests

Add snapshot and adversarial coverage for:

- single-root protocol machine
- multi-root machine
- sink states rendered to end markers
- branching transitions
- presented labels and descriptions
- composition machine with state-payload child references
- composition machine with machine-field child references
- composition machine with transition-param child references
- composition machine with detached attested handoff annotations

### Sequence Diagram Tests

Add coverage for:

- one direct transition-param handoff
- one attested handoff with one producer
- one attested handoff with multiple producers rendered as alternatives
- one composition path with multiple exact handoff steps
- one relation that is structural only and therefore emits a note instead of
  an arrow
- one branching composition machine where per-path export is required

### Semantic Authority Tests

Add adversarial cases for:

- `#[cfg]`-pruned transitions
- macro-generated transitions
- `include!()` boundaries where supported
- duplicate attested route identities
- one route name with multiple legal producer transitions
- direct child-machine relations from each source kind
- a composition path whose BFS machine reachability would differ from the true
  ordered transition path

## Implementation Phases

### Phase 0: Truth Audit

- add one module-level design note describing exact vs projected diagrams
- codify sink-state derivation
- codify sequence arrow eligibility by relation source kind

### Phase 1: Machine State Export

- add machine-local `stateDiagram-v2` renderer
- snapshot test protocol machines
- support labels from `ExportDoc`

### Phase 2: Codebase Machine State Export

- add codebase-machine state exporter
- support composition role labeling
- support exact composition annotations

### Phase 3: Relation Sequence Export

- add relation-detail sequence builder
- support direct and attested handoffs
- support multiple attested producers via explicit alternatives

### Phase 4: Composition Path Sequence Export

- enumerate ordered composition paths from machine transitions
- attach exact relation-backed sequence steps to those paths
- emit one file per selected path

### Phase 5: CLI And Inspector

- keep focused `state-diagram` and `sequence-diagram` commands stable
- add a diagram-first inspector shell
- add exact/projection badges
- add optional diagram bundle export from `export`

### Phase 6: `termaid` Preview Parity

- add preview capability where Rust `termaid` supports the Mermaid subset
- improve `termaid` composite-state support
- keep fallback-to-source behavior until parity is good enough

## Final Answer To The Core Question

Do we already have everything needed?

For protocol machine diagrams:

- yes

For composition machine diagrams as exact outer workflow diagrams:

- yes

For composition machine diagrams that attach exact protocol-machine and
detached-handoff evidence:

- yes

For exact nested diagrams that treat the whole child protocol graph as a true
substate of the composition machine:

- no, not as a current first-class truth surface

For exact sequence diagrams of exact handoffs and selected composition
transition paths:

- yes

For exact sequence diagrams of arbitrary end-to-end runtime behavior:

- no
