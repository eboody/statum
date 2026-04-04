# Composition Roadmap Checklist: Phase 4

This checklist is historical.

It documents the now-complete composition-first inspector milestone. The next
inspector iteration is tracked in:

- [Diagram-First Inspector Plan](../diagram-first-inspector-plan.md)
- [Statum Inspector TUI Spec](../inspector-tui-spec.md)

Goal at the time: inspector, atlas, and path UX.

- [x] Make composition machines the default home view when present.
- [x] Render composition machines as the primary workspace flow surface.
- [x] Add relationship cards that prefer composition-owned explanations.
- [x] Make path explorer prefer composition paths, then raw exact graph, then
      heuristic fallback.
- [x] Add a gaps view focused on missing composition modeling and older
      compatibility surfaces.
- [x] Keep exact, compatibility, and heuristic explanations visibly separate.
- [x] Keep graph bundle and JSON exact-only.
- [x] Update inspector docs to describe the composition-first mental model.

Acceptance gate:

- [x] Users can understand the main workspace flow from the inspector without
      starting in a declared journey view.
