# Composition Roadmap Checklist: Phase 6

Goal: cleanup, deprecation, and docs trim.

- [x] Remove stale journey-first APIs once composition parity is proven.
- [x] Remove journey resolver and TUI surfaces that no longer carry their
      weight.
- [x] Trim the attestation surface down to the minimum still needed for exact
      detached handoffs. `#[via(...)]`, binders, and `*_and_attest()` stay
      because they still solve a distinct exactness problem.
- [x] Remove compatibility paths that were kept only to bridge old transition
      syntax.
- [x] Delete stale roadmap docs, examples, and tutorial sections from earlier
      architectural directions.
- [x] Rewrite canonical docs so the composition model is the primary story.
- [x] Keep only the minimum compatibility surfaces that still solve a distinct
      exactness problem.

Acceptance gate:

- [x] The public API and docs are meaningfully smaller and clearer than before
      the migration.
