# Composition Roadmap Checklist: Phase 2

Goal: direct child-machine composition.

- [ ] Detect direct child-machine composition from state payloads on
      `role = composition` machines.
- [ ] Detect direct child-machine composition from composition-machine fields.
- [ ] Detect direct child-machine composition from composition transition
      parameters.
- [ ] Add composition-specific exact relation basis and detail to
      `CodebaseDoc`.
- [ ] Project composition-owned relations consistently through graph export,
      CLI, and inspector.
- [ ] Prefer composition-owned exact paths in path derivation.
- [ ] Add examples that show composition machines with no extra narrative DSL.
- [ ] Add adversarial tests proving non-composition machines do not get
      composition semantics by accident.

Acceptance gate:

- [ ] A composition machine carrying child machines is enough to define a
      top-level exact flow without `journeys!`.
