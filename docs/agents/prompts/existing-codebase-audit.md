# Existing Codebase Audit Prompt

Copy and paste this when you want an agent to scan a Rust codebase for strong
Statum opportunities.

```text
Audit this Rust codebase for places where Statum would materially improve
correctness, API clarity, or rebuild ergonomics.

Be conservative. Recommend Statum only when you find a staged entity with:
- finite named phases
- meaningful transition ordering
- phase-specific behavior or data
- duplicated runtime guard logic, or
- rebuild from rows, snapshots, or event logs

Start by inspecting status enums, boolean flag combinations, invalid transition
guards, state-specific data, and any row or event-log rebuild paths.

Return these sections:
## Strong candidates
## Maybe candidates
## Poor fits

For each strong candidate include:
- candidate entity
- concrete file and symbol evidence
- current runtime pain
- proposed `#[state]` enum
- proposed `#[machine]` fields
- state-specific data
- likely `#[transition]` blocks
- whether `#[validators]` or `statum::projection` fits
- smallest first migration slice
- migration risk and testing needs

Do not stop at "consider typestate." Give a concrete Statum sketch or say it is
a poor fit.

Use these references if needed:
- https://github.com/eboody/statum/blob/main/README.md
- https://github.com/eboody/statum/blob/main/docs/typestate-builder-design-playbook.md
- https://github.com/eboody/statum/blob/main/docs/patterns.md
- https://github.com/eboody/statum/blob/main/docs/persistence-and-validators.md
```
