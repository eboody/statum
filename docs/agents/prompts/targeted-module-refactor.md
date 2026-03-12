# Targeted Module Refactor Prompt

Copy and paste this when you already suspect one module or service should move
toward Statum.

```text
Inspect these Rust modules and decide whether they should be refactored to use
Statum:

- <path or module 1>
- <path or module 2>

Look at the module itself, its immediate call sites, and the tests around it.

If Statum is a good fit, return:
- the staged entity
- the proposed `#[state]` enum
- the `#[machine]` shared context
- the state-specific data
- the legal transitions and likely `#[transition]` blocks
- whether persisted rebuilds require `#[validators]` or `statum::projection`
- the smallest safe migration slice
- the tests that should move or be added

If Statum is not a good fit, explain exactly why and what should stay
runtime-validated instead.

Do not propose a full rewrite when a narrow first slice would prove the design.

Use these references if needed:
- https://github.com/eboody/statum/blob/main/README.md
- https://github.com/eboody/statum/blob/main/docs/typestate-builder-design-playbook.md
- https://github.com/eboody/statum/blob/main/docs/patterns.md
- https://github.com/eboody/statum/blob/main/docs/persistence-and-validators.md
```
