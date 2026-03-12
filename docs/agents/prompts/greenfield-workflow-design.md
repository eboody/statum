# Greenfield Workflow Design Prompt

Copy and paste this when you are designing a new Rust workflow and want the
agent to decide whether Statum should shape it.

```text
Help me design this Rust workflow. Use Statum only if it is a strong fit.

Context:
- entity: <name>
- domain: <feature or service>
- planned phases: <if known>
- persisted data or event log: <yes/no/details>
- current constraints: <latency, compliance, API shape, migration pressure>

Tasks:
1. Decide whether Statum is a strong fit, a maybe, or a poor fit.
2. If it is a strong fit or a maybe, propose:
   - a `#[state]` enum
   - a `#[machine]` struct with shared context
   - the state-specific data that should not live on the machine root
   - the likely `#[transition]` impl blocks
   - whether `#[validators]` or `statum::projection` is needed
3. If it is a poor fit, say what should stay runtime-validated and why.
4. Keep the first implementation slice small enough for one PR.

Be conservative. Do not recommend Statum for user-authored or rapidly changing
workflows.

Use these references if needed:
- https://github.com/eboody/statum/blob/main/README.md
- https://github.com/eboody/statum/blob/main/docs/typestate-builder-design-playbook.md
- https://github.com/eboody/statum/blob/main/docs/patterns.md
- https://github.com/eboody/statum/blob/main/docs/persistence-and-validators.md

Return:
## Fit
## Proposed shape
## First implementation slice
## Risks and open questions
```
