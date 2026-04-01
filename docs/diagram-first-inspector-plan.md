# Diagram-First Inspector Plan

This file is the forward-looking implementation plan for the next Statum
inspector.

It replaces the current text-first, atlas-first inspector direction with one
clear rule:

- the default `inspect` experience should be diagram-first
- Mermaid is the primary representation
- `termaid` is the primary terminal renderer
- lists and prose support the diagram instead of replacing it

The target user story is simple:

```text
cargo statum-graph inspect /path/to/workspace
```

The user should immediately see a useful visual representation of the exact
Statum structure in that workspace, without first exporting files or learning a
large set of list-navigation commands.

## Summary

Short answer:

- keep `inspect` as the main entrypoint
- keep exactness grounded in linked compiled Statum metadata
- make the center of the TUI a real diagram viewport
- make `Workspace`, `Machine`, and `Handoff` the primary modes
- make diagram bundles optional and additive, not required for the TUI

The best current experience for a workspace like `citacell` should be:

1. open on an exact workspace flow diagram
2. let the user pick a machine from the outline
3. render that machine as `stateDiagram-v2`
4. let the user drill into an exact relation or exact composition path
5. render that handoff as `sequenceDiagram`

Everything else should be secondary.

## Why The Current Inspector Is Not Enough

The current inspector improved navigation and visual treatment, but the product
shape is still wrong for the problem.

Current problems:

- diagrams are a detail tab instead of the main surface
- the shell is still list-first and card-first
- `Story`, `Machine`, and `Gaps` are useful, but they force users to infer the
  shape of the system from prose and rows before seeing the graph
- `state-diagram` and `sequence-diagram` exist as focused commands, but the
  inspector does not yet organize itself around those exact diagram surfaces
- there is no first-class plan object that says which diagram the TUI is
  showing and why
- there is no stable diagram bundle format for caching, docs, or downstream
  consumers

So the next step is not "add more diagram tabs." The next step is to make the
diagram the product.

## Product Direction

### Core Rule

The center pane should always answer:

- what exact diagram am I looking at right now

The outline and details should answer:

- why this diagram is selected
- what exact item inside it matters
- what supporting metadata exists

### Top-Level Modes

The next inspector should have three primary modes.

#### `Workspace`

Purpose:
exact workspace overview.

Primary diagram:

- exact workspace `flowchart`

Default behavior:

- if composition machines exist, start with a composition-focused workspace
  overview
- otherwise start with the exact workspace overview for all linked machines

Drilldown:

- selecting a machine enters `Machine`
- selecting an exact cross-machine edge enters `Handoff`

#### `Machine`

Purpose:
exact local protocol or exact composition-machine legality.

Primary diagram:

- exact `stateDiagram-v2` for one machine

Drilldown:

- state, transition, and validator details stay in support panes
- exact relation and exact path affordances can enter `Handoff`

#### `Handoff`

Purpose:
exact cross-machine understanding.

Primary diagram:

- exact `sequenceDiagram` for one selected exact relation
- later, exact `sequenceDiagram` for one selected exact composition path

Drilldown:

- source kind
- attested producer provenance
- target machine and target state
- source and route locations when available

### Secondary Surfaces

These should stay available, but not as the main product:

- raw Mermaid source
- source locations
- rustdoc and `#[present(...)]`
- deterministic explanation text
- heuristic gap and evidence views

`Gaps` should stop being a primary home view. It should become a secondary
diagnostic tray, overlay, or side-list that explains where the exact diagram
story is still incomplete.

## Authority Boundary

Claimed authority surface:

- exact workspace flowchart
- exact machine `stateDiagram-v2`
- exact relation `sequenceDiagram`
- later, exact composition-path `sequenceDiagram` only when the ordering is
  truly derivable

Actual observation points:

- exact machine-local topology:
  `MachineIntrospection::GRAPH` and `ExportDoc`
- exact linked workspace topology and exact relations:
  linked compiled `CodebaseDoc`
- exact composition ownership:
  composition roles and exact relation detail already extracted into linked
  metadata
- heuristic overlays:
  raw source scan over already-known exact machines

Unsupported and still out of scope:

- exact end-to-end runtime chronology across arbitrary workspace behavior
- exact full nested child-protocol hierarchy inside composition states
- heuristic-only diagrams presented as exact

Policy:

- unsupported exact diagrams fail closed
- heuristic views stay visibly weaker
- `termaid` is only a renderer, never a semantic source

## Architecture Changes Needed

The next TUI needs new stage-layer and leaf-layer structure. The current
inspector shape does not give the diagram enough first-class identity.

### 1. Add A First-Class Diagram Plan Surface

The TUI should stop asking ad hoc questions like "what tab is selected?" and
start asking "what diagram plan is active?"

Recommended stage-layer types:

- `WorkspaceDiagramPlan`
- `MachineDiagramPlan`
- `HandoffDiagramPlan`
- `DiagramPlan`
- `DiagramSurface`
- `DiagramSelection`

Those types should own:

- the exact semantic target
- the exactness level
- the Mermaid kind
- the exact renderer entrypoint
- the stable display title
- the stable source view identity

They should not own:

- ratatui layout
- `termaid` invocation
- heuristics that invent missing order or containment

### 2. Separate Semantic Selection From Viewport State

The session model should split:

- semantic selection:
  which machine, relation, or path is selected
- diagram selection:
  which exact Mermaid surface should render
- viewport state:
  horizontal scroll, vertical scroll, zoom or fit mode if added later

This avoids coupling list position to diagram identity.

### 3. Add Stable Diagram Keys

The current single-diagram commands are fine for stdout, but a diagram-first
inspector and optional bundle export need stable keys.

Recommended keys:

- machine diagram key:
  stable machine path-derived slug
- relation diagram key:
  exact relation identity plus a path-derived summary slug
- composition path diagram key:
  machine key plus a stable exact path selector

The key is for caching and bundle layout. It is not a claim that relation
indices are globally stable across arbitrary code edits.

### 4. Keep Mermaid Generation In Memory By Default

The inspector should not require temporary files just to preview a diagram.

Default path:

1. derive exact Mermaid string in memory
2. pass it to `termaid` on stdin
3. render stdout into the viewport
4. fall back to raw Mermaid source when preview is unavailable

Optional file output is still useful, but should be a separate surface.

### 5. Add An Optional Diagram Bundle

The TUI should not depend on a bundle, but the ecosystem will benefit from
one.

Recommended export layout:

- `diagrams/index.json`
- `diagrams/workspace/<scope>.flow.mmd`
- `diagrams/machines/<machine-key>.state.mmd`
- `diagrams/relations/<relation-key>.sequence.mmd`
- later `diagrams/composition/<machine-key>/<path-key>.sequence.mmd`

The manifest should include:

- diagram key
- Mermaid kind
- exactness label
- machine path or relation identity
- path selector when applicable
- human-facing title

Benefits:

- inspector can optionally reuse cached diagrams
- docs and PRs can consume stable `.mmd` outputs
- users can diff diagram output directly

### 6. Add Exact Composition-Path Sequence Export

The current path explorer ranks machine reachability, but it does not yet
export exact ordered composition paths as a first-class surface.

That needs:

- exact ordered composition path enumeration from machine transitions
- exact step attachment to real relation detail
- explicit rejection when a path only proves reachability, not order

This is the main blocker for a strong `Handoff` mode beyond single exact
relations.

### 7. Keep `termaid` Strictly As The Renderer

Statum owns:

- exact diagram identity
- exact Mermaid generation
- exact/projection labeling

`termaid` owns:

- terminal rendering of Mermaid

The inspector owns:

- choosing which diagram to show
- preview fallback behavior
- viewport behavior

Do not let the renderer become the authority surface.

## CLI Changes Needed

The current CLI is close, but not yet shaped for a diagram-first inspector.

### Keep These Stable

- `cargo statum-graph inspect`
- `cargo statum-graph state-diagram`
- `cargo statum-graph sequence-diagram`
- `cargo statum-graph export`

Those are already useful and should remain valid.

### Add These Later

#### `export --include-diagrams`

Recommended additive flags:

- `--include-workspace-diagrams`
- `--include-state-diagrams`
- `--include-sequence-diagrams`

This is the cleanest path to a reusable diagram bundle without forcing new
top-level commands on users.

#### Optional Diagram Namespace

An additive command family could later exist:

- `cargo statum-graph diagram workspace`
- `cargo statum-graph diagram state`
- `cargo statum-graph diagram sequence relation`
- `cargo statum-graph diagram sequence composition`

But this should be additive only. It should not replace the focused commands
until the namespace proves clearer in practice.

### `inspect` Behavior Changes

The user should not need extra flags to get the best default diagram view.

Recommended defaults:

- if composition machines exist:
  open `Workspace`
- if exactly one machine is strongly dominant for the selected package:
  make it the initial semantic selection
- otherwise:
  open the workspace overview with the highest-ranked machine selected in the
  outline

Potential additive flags:

- `--home workspace|machine|handoff`
- `--machine <path>`
- `--relation <index>`

These are useful, but should not be required for the normal case.

## TUI Shape

### Left Pane

Owns:

- outline
- search
- filters
- semantic selection list
- diagnostics badges

It should answer:

- what can I inspect next

### Center Pane

Owns:

- rendered `termaid` viewport
- raw Mermaid fallback view
- diagram identity chrome

It should answer:

- what exact diagram am I looking at

### Right Pane

Owns:

- structured detail for the currently selected semantic item
- docs
- source
- deterministic explanation

It should answer:

- what does the selected thing mean

### Bottom Bar

Owns:

- exactness
- mode
- viewport hints
- short key help

It should not be used as a content pane.

## Citacell Acceptance Story

For a workspace like `/home/eran/code/citacell`, the target experience is:

1. Run `cargo statum-graph inspect /home/eran/code/citacell/`.
2. The app opens on `Workspace`.
3. The center pane shows an exact workspace flowchart immediately.
4. The left outline highlights the highest-value composition machine or
   machine cluster.
5. Hitting `enter` opens that machine's exact `stateDiagram-v2`.
6. Selecting an exact cross-machine edge opens an exact `sequenceDiagram`.
7. If `termaid` is unavailable or cannot render the Mermaid subset, the
   diagram pane falls back to raw Mermaid with an explicit reason.

That is the bar. If the user still has to mentally reconstruct the graph from
list rows, the redesign is incomplete.

## Implementation Phases

### Phase 0: Docs And Boundary Cleanup

- rewrite stale inspector docs around the diagram-first model
- mark older atlas/path docs as historical where needed
- keep the exactness boundary explicit in docs

### Phase 1: Diagram Plan Layer

- add stage-layer diagram plan types
- add stable diagram identity and display metadata
- unify machine, relation, and future path selection under one diagram model

### Phase 2: Workspace Diagram Home

- make `Workspace` the default home surface
- render exact workspace flowchart in the center pane
- add outline-driven machine selection and drilldown
- move gaps to secondary diagnostics

### Phase 3: Machine Diagram Drilldown

- make `Machine` a first-class state-diagram mode
- keep state, transition, validator, and relation detail in support panes
- add viewport behavior for larger diagrams

### Phase 4: Handoff Diagram Drilldown

- add `Handoff` mode for exact relation sequence diagrams
- add source and provenance support panes
- keep heuristic handoffs visibly unavailable as exact diagrams

### Phase 5: Diagram Bundle Export

- add optional diagram bundle emission from `export`
- add manifest format and stable file layout
- allow inspector to reuse bundle outputs when available

### Phase 6: Exact Composition-Path Sequence Export

- add exact ordered composition-path selection
- emit exact sequence diagrams for selected paths
- connect `Handoff` mode to that surface

### Phase 7: Renderer And Viewport Polish

- improve `termaid` parity where needed
- add better scrolling and fit behavior
- decide whether richer semantic selection inside the diagram is possible or
  whether outline-driven selection remains the right model

## Checklist

### Product

- [ ] `inspect` opens on a diagram, not a text card
- [ ] the center pane is always the primary visual surface
- [ ] `Workspace`, `Machine`, and `Handoff` are the main modes
- [ ] diagnostics stay visible without becoming the primary home

### Authority

- [ ] exact diagrams are backed only by linked exact Statum surfaces
- [ ] heuristic diagrams are never presented as exact
- [ ] unsupported exact diagrams fail closed
- [ ] `termaid` remains presentation-only

### CLI And Export

- [ ] keep `inspect`, `state-diagram`, and `sequence-diagram` stable
- [ ] add optional diagram bundle export
- [ ] add stable diagram manifest keys
- [ ] keep file export optional for the TUI

### Implementation

- [ ] add first-class diagram plan types
- [ ] add viewport state separate from semantic selection
- [ ] make workspace flowchart the default home surface
- [ ] make machine state diagrams first-class drilldown
- [ ] add exact relation handoff drilldown
- [ ] add exact composition-path drilldown later

### Acceptance

- [ ] a new user can run `inspect` on a real workspace and immediately see a
      useful diagram
- [ ] a user can move from workspace to machine to handoff without losing
      orientation
- [ ] a user can still inspect raw Mermaid and source when needed
- [ ] the TUI remains useful when source-local presentation metadata is sparse
