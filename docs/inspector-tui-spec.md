# Statum Inspector TUI Spec

This file is the concrete product spec for the next inspector TUI.

It supersedes the earlier atlas-first, story-card-first inspector direction
with a diagram-first design:

- exact structure comes from compiled Statum metadata
- the inspector derives Mermaid from that exact structure
- `termaid` renders Mermaid in the terminal
- the diagram is the main UI surface
- lists, docs, and explanations support the diagram instead of replacing it

## Goals

- Make `cargo statum-graph inspect /path/to/workspace` immediately useful on a
  real workspace like `citacell`.
- Make the center pane a visual representation of the exact Statum surface,
  not a text summary of it.
- Keep exact and heuristic data visibly separate.
- Keep the default UX zero-touch: no handwritten graph files and no required
  extra export step.
- Make drilldown feel like moving through diagrams, not switching between
  unrelated lists.

## Non-Goals

- No runtime replay or runtime chronology claims.
- No manual narrative overlay file.
- No heuristic data in exact Mermaid, DOT, PlantUML, or JSON export.
- No claim of exact nested child-protocol hierarchy until Statum exports that
  truth surface.

## Authority Model

### Claimed Authority Surface

- exact workspace overview diagrams:
  exact within the linked compiled `CodebaseDoc` surface
- exact machine diagrams:
  exact within the linked machine topology surface
- exact handoff diagrams:
  exact for supported exact relations and later supported exact composition
  paths
- heuristic views:
  suggestive only, never exact

### Observation Points

- exact structure:
  macro-expanded, cfg-pruned linked compiled inventories collected into
  `CodebaseDoc`
- machine-local structure:
  `MachineIntrospection::GRAPH` and `ExportDoc`
- heuristic discovery:
  raw source scan over reachable module trees, limited to already-known exact
  machines
- source-local presentation:
  `#[present(...)]`, `#[presentation_types(...)]`, and rustdoc on compiled
  items

### Support Policy

- Exact diagrams fail closed when the exact surface is missing.
- Heuristic diagrams do not exist in the exact lane.
- If a detail is not present in the exact or extracted source-local surface,
  the inspector omits it or labels it heuristic.
- `termaid` is presentation-only. It does not strengthen a weaker semantic
  surface.

## Layering

This spec uses four layers.

### Narrative Layer

User-facing inspector modes:

- `Workspace`
- `Machine`
- `Handoff`
- support tabs for `Summary`, `Docs`, `Source`, `Mermaid`, and `Explain`

### Session Layer

Session-only derived behavior:

- semantic selection
- diagram selection
- search scope
- lane selection
- mixed-lane suppression
- viewport scroll state
- pane focus

### Protocol-Truth Layer

Authoritative data surfaces:

- linked `CodebaseDoc`
- machine-local graph export
- exact relation detail
- exact composition ownership
- exact attested route provenance

### Leaf Layer

Local mechanics only:

- ratatui rendering
- `termaid` invocation
- viewport scrolling
- text formatting
- key dispatch

## Primary Product Shape

The next inspector should have three primary modes.

### `Workspace`

Purpose:
show the exact workspace shape first.

Primary diagram:

- exact workspace `flowchart`

Default opening:

- if composition machines exist, bias selection and ranking toward them
- otherwise show the exact linked workspace overview

Drilldown:

- selecting a machine opens `Machine`
- selecting an exact cross-machine handoff opens `Handoff`

### `Machine`

Purpose:
show one machine's exact legality.

Primary diagram:

- exact `stateDiagram-v2`

Applies to:

- protocol machines
- composition machines as outer workflow truth

Drilldown:

- selected state
- selected transition
- validator details
- exact relation and path affordances that can open `Handoff`

### `Handoff`

Purpose:
show one exact cross-machine interaction.

Primary diagram:

- exact `sequenceDiagram` for one exact relation
- later, exact `sequenceDiagram` for one exact composition path

Drilldown:

- source kind
- attested producer provenance
- target machine and target state
- route identity
- source locations when available

## Secondary Product Shape

These are required, but secondary:

- raw Mermaid source
- source locations
- deterministic explanation text
- docs and rustdoc
- diagnostics and heuristic evidence

`Gaps` should stop being a primary top-level mode. Diagnostic and heuristic
surfaces should still exist, but as support views within the current semantic
selection.

## Layout Contract

### Left Pane

Owns:

- outline
- search
- filters
- semantic selection list
- diagnostic badges

It answers:

- what can I inspect next

### Center Pane

Owns:

- diagram viewport
- exactness badge
- diagram title
- raw Mermaid fallback view

It answers:

- what exact diagram am I looking at

### Right Pane

Owns support tabs:

- `Summary`
- `Docs`
- `Source`
- `Mermaid`
- `Explain`

It answers:

- what does the selected semantic item mean

### Bottom Bar

Owns:

- mode
- exactness
- search scope
- viewport hints
- short key help

It must not become a content pane.

## Selection Model

The inspector should separate three things.

### Semantic Selection

Examples:

- selected machine
- selected exact relation
- selected exact composition path
- selected state or transition support item

### Diagram Selection

Examples:

- workspace flowchart for current scope
- machine state diagram for one machine
- relation sequence diagram for one relation

### Viewport State

Examples:

- horizontal scroll
- vertical scroll
- future fit mode or zoom mode

The TUI should not derive diagram identity indirectly from ad hoc list state.

## Home View Rules

### Startup

1. Link the target workspace.
2. Build the exact workspace diagram surface.
3. Rank initial machine selection.
4. Open in `Workspace`.

### Ranking

If composition machines exist, rank by:

1. composition role
2. exact cross-machine degree
3. stable machine path

Otherwise rank by:

1. exact cross-machine degree
2. stable machine path

## Search And Filters

Search still matters, but it should support diagram selection rather than act
as the main UI.

### Search Scopes

- `primary`
- `docs`
- `relations`
- `paths`
- `all`

### Search Rules

- active scope stays visible
- selected outline rows show why they matched
- search never silently widens itself
- empty states say whether the miss came from query, lane, or filters

### Filter Rules

Exact filters may include:

- relation kind
- relation basis
- composition-only
- inbound-only
- outbound-only

Heuristic filters may include:

- signature evidence
- body evidence
- shadowed-by-exact visibility

Filter state must remain visible but compact.

## Viewport Rules

The diagram viewport is the product center, so it needs explicit rules.

### Default Render Path

1. derive exact Mermaid in memory
2. send Mermaid to `termaid`
3. render terminal output in the viewport
4. if rendering fails, show raw Mermaid with an explicit reason

### Viewport Behavior

- support vertical scrolling
- support horizontal scrolling
- keep semantic selection stable while scrolling
- do not rebuild semantic selection from scroll position

### Fallback Behavior

- if Mermaid generation fails closed, say why
- if `termaid` is unavailable, say why
- if `termaid` cannot render the Mermaid subset, show raw Mermaid
- do not silently down-convert one exact surface into a weaker approximate
  diagram

## Support Tabs

### `Summary`

Deterministic structured facts for the selected semantic item.

Examples:

- machine role
- state count
- transition count
- relation source kind
- attested route identity
- exactness label

### `Docs`

Source-local enrichment:

- `#[present(description = ...)]`
- rustdoc
- other extracted source-local presentation data

### `Source`

Exact items should show, when available:

- machine path
- state name
- transition method
- exact relation source kind
- attested route identity
- source file and line for exact relation records

Heuristic items should show:

- file path
- line
- evidence kind
- matched path text
- snippet

### `Mermaid`

Always show the raw Mermaid source that underlies the current diagram plan.

This is required even when `termaid` preview works.

### `Explain`

Deterministic generated prose from exact structure.

Rules:

- grounded in structure
- grounded in source-local metadata when present
- no invented business intent
- if the current lane is heuristic, say so

## Citacell Acceptance Story

For `/home/eran/code/citacell`, the target experience is:

1. run `cargo statum-graph inspect /home/eran/code/citacell/`
2. see an exact workspace flowchart immediately
3. select the most important machine from the outline
4. see that machine's exact `stateDiagram-v2`
5. select an exact handoff
6. see an exact `sequenceDiagram`
7. inspect raw Mermaid or source only when needed

If the user still has to reconstruct the architecture from text rows before
seeing the graph, the product is missing the point.

## Source-Local Metadata Policy

Source-local metadata is useful, but optional.

Supported:

- `#[present(label = ...)]`
- `#[present(description = ...)]`
- typed `#[presentation_types(...)]` metadata
- rustdoc

Policy:

- the inspector remains useful without it
- metadata enriches exact items
- metadata does not gate the diagram-first UX
- no separate inspector-only config file is required

## Out Of Scope For This Spec

- runtime replay integration
- persistence snapshots
- external operational runbooks
- team ownership dashboards
- alerts and timers not modeled in source-local typed metadata
- exact nested child-slot hierarchy until Statum exports it as a first-class
  truth surface

## Acceptance Criteria

- A user can understand the workspace shape from the initial `Workspace`
  diagram.
- A user can move from workspace to machine to handoff without losing
  orientation.
- A user can tell which surfaces are exact, heuristic, or source-local.
- A user can inspect raw Mermaid and exact source status without leaving the
  current semantic selection.
- The inspector remains useful when source-local metadata is sparse or absent.

## Implementation Checklist

### Authority And Data Surface

- [ ] keep exact diagrams backed only by linked compiled exact surfaces
- [ ] keep heuristic evidence out of exact Mermaid
- [ ] fail closed for unsupported exact diagrams
- [ ] document exact, heuristic, and presentation observation points in code
      and docs

### View Model

- [ ] add primary modes: `Workspace`, `Machine`, `Handoff`
- [ ] add support tabs: `Summary`, `Docs`, `Source`, `Mermaid`, `Explain`
- [ ] separate semantic selection, diagram selection, and viewport state
- [ ] keep diagnostics secondary instead of top-level

### Diagram Viewport

- [ ] make the center pane a real diagram viewport
- [ ] support `termaid` preview with raw Mermaid fallback
- [ ] add scroll behavior for larger diagrams
- [ ] keep semantic selection stable while scrolling

### Diagram Selection

- [ ] add exact workspace flowchart home view
- [ ] add exact machine `stateDiagram-v2` drilldown
- [ ] add exact relation `sequenceDiagram` drilldown
- [ ] add exact composition-path `sequenceDiagram` later

### Navigation

- [ ] keep `q` as the direct quit key
- [ ] keep `?` help
- [ ] keep search scoped and legible
- [ ] make mode switches diagram-oriented instead of pane-tab oriented
- [ ] keep viewport controls discoverable in help and bottom chrome

### Source And Docs

- [ ] keep source file and line for heuristic evidence
- [ ] surface exact source location status where available
- [ ] keep attested route provenance visible in `Summary` and `Source`
- [ ] keep raw Mermaid always available in `Mermaid`

### Documentation

- [ ] keep this spec aligned with the Mermaid plan
- [ ] keep the inspector plan pointer aligned with this spec
- [ ] update `cargo-statum-graph/README.md` only when shipped behavior changes
- [ ] narrow any wording that promises more authority than the implementation
      can justify
