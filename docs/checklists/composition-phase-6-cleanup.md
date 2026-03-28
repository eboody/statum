# Composition Roadmap Checklist: Phase 6

Goal: cleanup, deprecation, and docs trim.

- [ ] Deprecate stale journey-first APIs once composition parity is proven.
- [ ] Remove journey resolver and TUI surfaces that no longer carry their
      weight.
- [ ] Deprecate or remove superseded `#[via(...)]` helpers and binder APIs
      after detached-handoff migration is complete.
- [ ] Remove compatibility paths that were kept only to bridge old transition
      syntax.
- [ ] Delete stale roadmap docs, examples, and tutorial sections from earlier
      architectural directions.
- [ ] Rewrite canonical docs so the composition model is the primary story.
- [ ] Keep only the minimum compatibility surfaces that still solve a distinct
      exactness problem.

Acceptance gate:

- [ ] The public API and docs are meaningfully smaller and clearer than before
      the migration.
