# Diagram-First Inspector Plan

This file is historical.

The inspector is no longer being driven by the older map-first,
generic-diagram-shell plan. The canonical product and implementation spec now
lives in [inspector-tui-spec.md](./inspector-tui-spec.md).

Why this changed:

- the old plan made `Topology` the first thing users saw
- composition traces were still being told through relation or sequence
  surfaces instead of direct composition-state progression
- the remaining navigation model was still too generic and modal

The current direction is:

- `Journeys` first
- exact composition trace projection as the main story surface
- `Machines` for legality drilldown
- `Topology` for context
