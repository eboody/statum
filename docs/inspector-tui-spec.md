# Statum Inspector TUI Spec

This is the canonical inspector product spec.

It replaces the older map-first and generic diagram-shell plans. The inspector
is now journey-first:

- `Journeys` is the default home when a workspace has any composition machine
- one selected finite root-to-sink composition trace is the main story surface
- the center pane shows an exact Mermaid `stateDiagram-v2` projection for that
  selected journey
- `Machines` and `Map` remain available, but as secondary views

## Product Goal

For one composition machine, the inspector should answer:

- what happens from entry to exit
- in what order
- through which composition states and transitions
- which protocol machines or states are touched along the way

## Non-Goals

- no whole-workspace runtime chronology
- no heuristic Mermaid diagrams presented as exact
- no exact nested child-machine hierarchy inside composition states until the
  extractor exports that truth surface
- no external narrative overlay file as a required input

## Authority Model

### Claimed Authority Surface

- `Journeys` shows one exact finite root-to-sink composition trace for one
  selected composition machine
- `Machines` shows one exact machine-local legality diagram
- `Map` shows exact linked machine topology
- relation drilldown can show one exact relation `sequenceDiagram`

### Observation Point

- linked compiled `CodebaseDoc` for exact workspace, composition, and relation
  surfaces
- machine-local exported topology for full machine diagrams
- source-local presentation only from compiled rustdoc and `#[present(...)]`
  metadata
- `termaid` is presentation-only

### Explicit Non-Claims

- no exact nested child-machine hierarchy inside composition states
- no exact runtime chronology inside touched protocol machines
- no silently chosen producer when the exact relation surface exposes several
  attested producers
- no partial exact journey listing after budget failure

## Identity And Enumeration

### Journey Identity

- display labels use `Entry -> Exit`
- internal identity uses a snapshot-scoped `JourneyId`
- `JourneyId` is derived from the selected machine plus the ordered exact step
  sequence: ingress state and ordered `(transition_index, to_state)` pairs
- if two journeys share the same endpoints, they remain distinct because the
  step sequence differs

### Enumeration Policy

- enumeration is deterministic
- one DFS expansion is counted per recursive enumerator frame after outgoing
  steps are sorted deterministically
- phase-1 limits:
  - `MAX_EXACT_JOURNEYS = 256`
  - `MAX_DFS_EXPANSIONS = 16_384`
- if either limit is exceeded, journey listing fails closed with
  `TooManyJourneys`
- no partial exact list is shown after budget failure

### Grouping Policy

- if enumeration succeeds and the machine has at most `64` exact journeys,
  list them directly
- if enumeration succeeds and the machine has `65..256` exact journeys, the
  target UI is grouped `Entry -> Exit` families with exact variant drilldown
- grouped families must be derived from the full exact journey set, never from
  truncation or sampling

Phase note:

- phase-1 shipped the deterministic budgets and fail-closed behavior
- grouped-family UI is a follow-up, not a relaxed semantic claim

## Top-Level Modes

### Journeys

Purpose:

- composition-story home

Layout:

- left top: composition machine list
- left bottom: journey list for the selected machine
- center top: context header
- center body: exact journey projection diagram
- right: support tabs, defaulting to `Steps`

Context header always shows:

- selected machine
- journey count or journey-status message
- selected journey label
- touched protocol summary
- a `Map` jump hint

Journey list rows show:

- `Entry -> Exit` label using state labels
- step count
- short touched-protocol summary

Zero-step journeys are valid:

- row subtitle: `entry and exit in <state> • 0 steps`
- diagram: one state with `[*] --> state` and `state --> [*]`
- `Steps` tab: `No composition transitions. This journey enters and exits in <state>.`

### Machines

Purpose:

- legal-state drilldown

Layout:

- left: machine list
- center: exact full-machine `stateDiagram-v2`
- right: `Summary`, `States`, `Transitions`, `Relations`, `Journeys` for
  composition machines, `Docs`, `Source`, `Mermaid`, `Issues`

### Map

Purpose:

- workspace context

Layout:

- center: exact workspace `flowchart`
- map opens around the selected machine when entered from `Journeys`
- scales:
  - `Overview`
  - `Focus`
  - `Full`

Map is context, not the default home on composition-heavy workspaces.

## Diagram Rules

### Journey Diagram

- Mermaid `stateDiagram-v2`
- only the selected journey's states and transitions
- `[*]` ingress and egress markers
- numbered transition labels like `1. start`
- no fake nested child-machine Mermaid states

Protocol involvement belongs in the right pane, not inside the exact journey
diagram.

### Machine Diagram

- Mermaid `stateDiagram-v2`
- full exact machine-local legality

### Map Diagram

- Mermaid `flowchart`
- composition machines as double boxes
- protocol machines as plain boxes
- owned orchestration handoffs as thick arrows
- other linked handoffs as solid arrows
- static references as dotted arrows

## Right-Pane Tabs

### Journeys Mode

- `Steps`: ordered composition steps with state change, zero or more touched
  protocol targets, carried-state notes, and producer-alternative notes when
  the exact relation surface exposes them
- `Protocols`: grouped summary of every protocol machine touched by the
  selected journey
- `Mermaid`: raw Mermaid source for the current journey projection
- `Source`: exact file or line locations when available
- `Issues`: fail-closed explanation for unavailable journey surfaces and any
  weaker heuristic context

### Machine And Map Modes

- keep the existing guide/docs/source/mermaid/explain split, but match the
  selected semantic subject instead of reusing journey wording

## Navigation Contract

- `q` quits
- `Esc` backs out to the outline or clears search, never quits
- `Tab` and `Shift-Tab` move focus
- in `Journeys`, focus order is:
  - machines
  - journeys
  - diagram
  - detail
- `j` / `k` and arrows act on the currently focused list or viewport only
- `h` / `l` pan the diagram when the center viewport is focused
- `[` / `]` switch center or detail tabs
- `1`, `2`, `3` switch `Journeys`, `Machines`, `Map`

The inspector should never require a temporary `pick vs scroll` mode.

## Naming Rules

Prefer this order:

- `#[present(label = ...)]`
- short doc-derived label when already available
- compact type label such as `outbound_release::Flow<State>` only as a last
  resort

Primary chrome should say:

- `Journeys`
- `Machines`
- `Map`
- `Steps`
- `Protocols`
- `Source`
- `Issues`

Primary chrome should not say:

- `checkpoint`
- `comp`
- `exact lane`
- `heuristic lane`
- `mixed lane`

Those terms belong in diagnostics, not the main story surface.

## CLI Contract

Stable commands:

- `inspect`
- `state-diagram`
- `sequence-diagram`
- `suggest`

Inspector defaults:

- if the workspace has composition machines, `inspect` opens on `Journeys`
- otherwise it opens on `Machines`

Optional inspector deep links are useful for demos and tests:

- `--mode journeys|machines|map`
- `--machine <selector>`

Phase note:

- public `--journey` deep links should wait until the internal `JourneyId`
  encoding is intentionally frozen

## Mermaid And Termaid Boundary

- Statum generates Mermaid from exact linked data
- `termaid` renders Mermaid in the terminal
- if `termaid` is unavailable, the inspector shows raw Mermaid source and says
  why
- if `termaid` cannot render the Mermaid subset, the inspector falls back to
  raw Mermaid instead of approximating

## Acceptance Scenarios

On a workspace like `citacell`:

- opening `inspect` lands on one composition machine and one concrete journey
- the selected journey answers the order of composition states and transitions
  without first decoding the workspace map
- protocol touches are visible in the right pane without inventing protocol
  runtime chronology
- moving to `Map` feels like context
- moving to `Machines` feels like legality drilldown

## Test Obligations

Renderer and authority:

- journey projection renders only the selected journey's states and transitions
- same-endpoint journeys have distinct `JourneyId` values
- zero-step journeys render correctly
- missing-root machines fail closed
- reachable cycles fail closed
- over-budget enumeration fails closed with no partial list
- no fake nested child-machine hierarchy appears in journey Mermaid

Inspector interaction:

- composition workspaces open to `Journeys`
- protocol-only workspaces open to `Machines`
- `Journeys` focus order is machine list -> journey list -> diagram -> detail
- diagram scrolling and journey selection never fight over the same focus
- map scale, hop radius, and layout keys only affect `Map`

## Delivery Order

1. deterministic journey identity, budgets, and exact journey projection
2. journey-first inspector shell and focus model
3. map and machine drilldown alignment around preserved selection
4. grouped journey-family UI for large but still exact acyclic machines
5. docs and README alignment
