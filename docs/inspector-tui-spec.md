# Statum Inspector TUI Spec

This file is the concrete product and implementation spec for the next
inspector TUI.

It replaces ad hoc inspector discussion with one derive-first design:

- exact structure comes from compiled Statum metadata
- the inspector derives as much as possible from that structure
- heuristics stay visible but clearly weaker
- source-local metadata is optional enrichment, not the primary content model
- external inspector-only config is out of scope by default

## Goals

- Make the inspector update when code changes, without a parallel handwritten
  graph file.
- Make composition machines the default workspace story when they exist.
- Keep exact and heuristic lanes visibly separate.
- Reduce modal navigation and make the TUI learnable on first use.
- Make drilldown useful for both architecture and code navigation.

## Non-Goals

- No runtime replay or runtime truth claims in this spec.
- No manual journey DSL.
- No external workspace-owned overlay file by default.
- No heuristic data in exported exact JSON, Mermaid, DOT, or PlantUML output.

## Authority Model

This section is the semantic boundary for the inspector.

### Claimed Authority Surface

- Exact lane: exact within the linked compiled Statum surface used by
  `CodebaseDoc::linked()`.
- Heuristic lane: suggestive only, never exact.
- Presentation lane: optional source-local labels, descriptions, typed
  metadata, and rustdoc attached to exact items.

### Observation Points

- Exact structure:
  macro-expanded, cfg-pruned linked compiled inventories collected into
  `CodebaseDoc`.
- Heuristic discovery:
  raw source scan over reachable library module trees, limited to already-known
  exact machines.
- Source-local presentation:
  `#[present(...)]`, `#[presentation_types(...)]`, and outer rustdoc on
  compiled items.

### Support Policy

- Exact lane unsupported cases must fail closed.
- Heuristic lane may be partial or unavailable, but it must say so explicitly.
- Heuristic results must never be merged into exact truth.
- If a detail is not present in the exact or source-local extracted surface,
  the inspector must omit it or label it heuristic. It must not guess.

## Layering

This spec uses four layers.

### Narrative Layer

The user-facing inspector surfaces:

- `Story`
- `Machine`
- `Gaps`
- `Source`
- `Explain`

This layer tells the workspace story, but does not redefine graph legality.

### Session Layer

Session-only derived behavior:

- search scope
- lane selection
- mixed-lane overlap suppression
- path ranking
- machine ranking
- focus state
- selection state

This layer combines exact truth with optional heuristic overlays.

### Protocol-Truth Layer

Authoritative data surfaces:

- linked `CodebaseDoc`
- exact relation groups
- composition diagnostics built from exact plus heuristic analysis
- validator-entry inventories
- attested route provenance

This layer owns exact legality and exact exported structure.

### Leaf Layer

Local mechanics only:

- ratatui rendering
- text formatting
- key dispatch
- sorting
- highlight rendering
- snippet display

## Data Sources

The next inspector should consume these sources in priority order.

### Exact Derived Data

- machines
- states
- transitions
- machine roles
- validator entries
- exact relations
- grouped machine summary edges
- direct-construction availability
- attested producer route detail
- exact path graph

### Heuristic Derived Data

- heuristic machine-to-machine couplings
- source snippets
- file and line evidence
- heuristic-only gap suggestions

### Optional Source-Local Enrichment

- `#[present(label = ...)]`
- `#[present(description = ...)]`
- typed `#[presentation_types(...)]` metadata
- rustdoc on machines, states, transitions, and validator impls

### Not Derived By Default

These are not part of the default inspector data model unless Statum later
extracts them from source-local typed metadata:

- owner/team
- runbook
- pager
- SLA or timeout semantics
- business criticality
- side effects not represented in typed machine structure

## Derivation Rules

These rules define how the inspector should turn extracted data into the TUI.

### Home View Selection

1. If any composition machines exist, open in `Story`.
2. Rank story entries by:
   composition root status,
   outbound exact cross-machine degree,
   total exact cross-machine degree,
   stable machine path.
3. If no composition machines exist, open in `Machine`.

### Label Selection

1. `#[present(label = ...)]`
2. existing human-facing display label derived by Statum
3. Rust type path or Rust item name

### Summary Copy Selection

1. `#[present(description = ...)]` for short copy
2. rustdoc for long-form detail
3. deterministic generated summary from exact structure

### Path Ranking

1. composition-owned exact paths
2. non-composition exact paths
3. heuristic fallback paths when the lane allows them

Within each class:

1. shortest hop count
2. stable target-machine path
3. stable path label

### Mixed-Lane Suppression

- If an exact relation covers the same source machine, optional source
  state-or-transition, and target machine, hide the heuristic duplicate in
  mixed mode.
- Mixed mode may annotate that hidden heuristic evidence existed, but it must
  not replace the exact card.

### Diagnostics

- Exact warning:
  protocol machine already exposes exact typed orchestration and likely wants
  `role = composition`.
- Heuristic suggestion:
  source scan shows coupling not yet modeled in exact composition surfaces.

## Top-Level Views

The next inspector should have three stable top-level views.

### Story

Purpose:
top-level workspace story derived from composition machines.

Left pane:

- story outline of composition machines
- exact edge count
- heuristic edge count
- gap badge count

Center pane:

- overview card for the selected composition machine
- preferred outgoing paths
- summary edges
- state and transition preview list
- diagnostics badges

Right pane tabs:

- `Summary`
- `Docs`
- `Source`
- `Explain`

Default selection:

- selected composition machine
- `Summary` tab

### Machine

Purpose:
leaf protocol drilldown for one machine.

Left pane:

- machine outline
- machine search result list

Center pane tabs:

- `Overview`
- `States`
- `Transitions`
- `Validators`
- `Relations`
- `Paths`
- `Diagnostics`

Right pane tabs:

- `Summary`
- `Docs`
- `Source`
- `Explain`

Rules:

- `Overview` is the default tab.
- `Relations` shows exact, heuristic, or mixed data according to lane.
- `Paths` shows best visible paths from the selected machine.
- `Diagnostics` shows machine-local composition warnings or suggestions.

### Gaps

Purpose:
triage view for what still relies on heuristics or weaker exact modeling.

Left pane:

- gap list
- severity
- source machine
- target machine

Center pane:

- gap card
- best currently visible path to target
- evidence counts

Right pane tabs:

- `Summary`
- `Docs`
- `Source`
- `Explain`

Rules:

- `Gaps` is secondary. Story view should already surface badge-level gap
  signals without requiring a mode switch.

## Pane Contract

The pane roles should stay stable across views.

### Left Pane

- outline
- current list
- search result list
- filters

### Center Pane

- active content view
- tabs within the selected top-level view

### Right Pane

Inspector tabs for the current selection:

- `Summary`:
  deterministic structured facts
- `Docs`:
  description plus rustdoc
- `Source`:
  source locations, snippets, route provenance, and jump targets
- `Explain`:
  deterministic plain-language explanation generated from exact structure

### Bottom Bar

- current view
- current lane
- current search scope
- current filter summary
- key hints for the current mode

The bottom bar must not become a major content pane.

## Navigation Spec

The next inspector should remove cycle-heavy navigation.

### Global Rules

- `q`: quit
- `?`: open help overlay
- `/`: open search
- `esc`: back out of the current modal state or clear search
- `enter`: drill into the selected item
- `tab` and `shift-tab`: move between panes

### Direct Top-Level Selection

- `1`: `Story`
- `2`: `Machine`
- `3`: `Gaps`

### Direct Lane Selection

- `e`: exact
- `m`: mixed
- `h`: heuristic

### List Navigation

- arrow keys
- `j` and `k`
- `home` and `end`
- `pageup` and `pagedown`

### Tab Navigation

- `[` previous tab
- `]` next tab

## Search Spec

Search should become scoped and legible.

### Default Scope

- names
- labels
- short descriptions

### Optional Scopes

- `docs`
- `relations`
- `paths`
- `all`

### Search Rules

- The active scope must stay visible while searching.
- The selected row must show why it matched.
- Search must not silently widen itself to all scopes.
- Empty search result states should explain whether the miss came from query,
  lane, or active filters.

## Filters

Filters should stay explicit and lane-aware.

### Exact Filters

- relation kind
- relation basis
- composition-only toggle
- inbound-only toggle
- outbound-only toggle

### Heuristic Filters

- signature evidence
- body evidence
- shadowed-by-exact visibility

### Filter Rules

- Filter state must remain visible in the bottom bar.
- `0` may still clear filters, but direct filter toggles must also be
  discoverable from the help overlay.

## Source Tab

The `Source` tab is required for the next inspector.

### Exact Items

Show, when available:

- machine path
- state name
- transition method
- exact relation source kind
- attested route identity
- source file and line for exact relation records
- future machine, state, and transition definition locations once extracted

### Heuristic Items

Show:

- file path
- line
- evidence kind
- matched path text
- snippet

### Source Tab Rules

- Do not claim a jump target that the extracted surface does not provide.
- If an exact item has no extracted location yet, say `source location not
  available`.

## Explain Tab

`Explain` is deterministic generated prose from exact structure. It is not
hand-authored narrative and it is not an LLM feature.

Examples:

- `WorkflowMachine is a composition machine with 4 states, 3 transitions, 2
  outbound exact cross-machine edges, and 1 composition warning.`
- `start_shipping accepts an attested handoff from PaymentMachine::capture and
  targets FulfillmentState::Shipping.`

Rules:

- Prefer counts, roles, paths, and attested provenance.
- Do not invent business intent not present in the structure or source-local
  metadata.
- If the current lane is heuristic, say so.

## Source-Local Metadata Policy

Source-local metadata is allowed and useful, but optional.

### Supported Metadata

- `#[present(label = ...)]`
- `#[present(description = ...)]`
- typed `#[presentation_types(...)]` metadata
- rustdoc

### Policy

- The inspector must remain useful when none of this metadata exists.
- Metadata should enrich exact items, not gate the base UX.
- The next implementation should not require a separate inspector-only config
  file to become useful.

## Out of Scope For This Spec

- runtime replay integration
- persistence snapshots
- external operational runbooks
- team ownership dashboards
- alerts and timers not modeled in source-local typed metadata

## Acceptance Criteria

- A user can understand the top-level workspace story from `Story` without
  starting in a leaf-machine list.
- A user can tell which facts are exact, heuristic, or source-local metadata.
- A user can search without guessing what scope is active.
- A user can inspect an exact relation and see route provenance plus source
  location status.
- The inspector remains useful with zero manual inspector-only annotations.

## Implementation Checklist

### Authority And Data Surface

- [ ] Document exact, heuristic, and source-local observation points in code
      and user docs.
- [ ] Keep exact export and exact inspector cards backed only by linked
      compiled `CodebaseDoc`.
- [ ] Keep heuristic evidence TUI-only.
- [ ] Add explicit source-location fields for exact relation records in the
      TUI model where missing.
- [ ] Add extracted definition locations for machines, states, and transitions
      if the current linked surface does not expose them yet.

### View Model

- [ ] Add stable top-level views: `Story`, `Machine`, `Gaps`.
- [ ] Add stable right-pane tabs: `Summary`, `Docs`, `Source`, `Explain`.
- [ ] Add machine-center tabs: `Overview`, `States`, `Transitions`,
      `Validators`, `Relations`, `Paths`, `Diagnostics`.
- [ ] Make `Story` the default when composition machines exist.
- [ ] Keep `Machine` as the fallback default when composition machines do not
      exist.

### Derivation Rules

- [ ] Implement story ranking from composition roots and exact cross-machine
      degree.
- [ ] Implement deterministic path ranking with
      composition > exact > heuristic precedence.
- [ ] Keep mixed-lane heuristic suppression tied to exact cover.
- [ ] Generate deterministic `Explain` copy from structure and metadata.

### Navigation

- [ ] Remove global `Esc` quit behavior.
- [ ] Make `q` the only direct quit key.
- [ ] Add help overlay on `?`.
- [ ] Add direct top-level view selection.
- [ ] Add direct lane selection.
- [ ] Keep pane focus movement stable across views.

### Search And Filters

- [ ] Add visible search scope.
- [ ] Add match-reason rendering for the selected result.
- [ ] Keep filter state visible in the bottom bar.
- [ ] Separate exact filters from heuristic filters in the UI.
- [ ] Improve empty-state copy so it names the active blockers:
      query, lane, or filters.

### Story View

- [ ] Build story outline from composition machines.
- [ ] Show exact edge counts, heuristic edge counts, and gap badges in the
      outline.
- [ ] Show preferred outgoing paths and summary edges in the center pane.
- [ ] Surface machine-local diagnostics without forcing a switch to `Gaps`.

### Machine View

- [ ] Make `Overview` the default machine tab.
- [ ] Keep states, transitions, validators, relations, paths, and diagnostics
      as direct drilldown tabs.
- [ ] Ensure exact and heuristic relation views stay visually distinct.
- [ ] Keep machine view useful even without source-local metadata.

### Gaps View

- [ ] Keep `Gaps` as a focused triage screen rather than the primary home.
- [ ] Show best visible path to the target machine.
- [ ] Show evidence counts and why-text.
- [ ] Make gap badges discoverable from `Story`.

### Source And Docs

- [ ] Show source file and line for heuristic evidence.
- [ ] Show source location status for exact items.
- [ ] Show attested route provenance in `Source` and `Summary`.
- [ ] Keep docs rendering separate from structured summaries.

### Documentation

- [x] Link this spec from the inspector plan pointer page.
- [ ] Update `cargo-statum-graph/README.md` after implementation so public
      keybindings and pane descriptions match the shipped UI.
- [ ] Narrow any wording that promises more authority than the implemented
      observation point supports.
