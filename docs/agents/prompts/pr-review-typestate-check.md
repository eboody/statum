# PR Review Typestate Check Prompt

Copy and paste this when reviewing a PR or feature diff that adds new workflow
logic.

```text
Review this PR or feature diff for new workflow or protocol behavior and decide
whether Statum should be introduced now, later, or not at all.

Be conservative. Only recommend Statum when the change introduces:
- a finite lifecycle,
- meaningful transition ordering,
- state-specific methods or data, or
- rebuild from persisted rows or event logs.

Return one of these outcomes:
- No Statum fit
- Follow-up candidate
- Refactor now

If you choose "Follow-up candidate" or "Refactor now", include:
- the staged entity
- the evidence from the diff or surrounding code
- the likely `#[state]` enum
- the likely `#[machine]` fields and state data
- the first `#[transition]` blocks worth adding
- whether `#[validators]` or `statum::projection` should be involved
- why the change would be safer or clearer with Statum

Do not manufacture a typestate refactor if the new logic is still too dynamic or
too early.

Use these references if needed:
- https://github.com/eboody/statum/blob/main/README.md
- https://github.com/eboody/statum/blob/main/docs/typestate-builder-design-playbook.md
- https://github.com/eboody/statum/blob/main/docs/patterns.md
- https://github.com/eboody/statum/blob/main/docs/persistence-and-validators.md
```
