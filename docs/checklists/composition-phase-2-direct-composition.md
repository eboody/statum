# Composition Roadmap Checklist: Phase 2

Goal: direct child-machine composition.

- [x] Detect direct child-machine composition from state payloads on
      `role = composition` machines.
- [x] Detect direct child-machine composition from composition-machine fields.
- [x] Detect direct child-machine composition from composition transition
      parameters.
- [x] Add composition-specific exact relation basis and detail to
      `CodebaseDoc`.
- [x] Project composition-owned relations consistently through graph export,
      CLI, and inspector.
- [x] Prefer composition-owned exact paths in path derivation.
- [x] Add examples that show composition machines with no extra narrative DSL.
- [x] Add adversarial tests proving non-composition machines do not get
      composition semantics by accident.

Acceptance gate:

- [x] A composition machine carrying child machines is enough to define a
      top-level exact flow without `journeys!`.
