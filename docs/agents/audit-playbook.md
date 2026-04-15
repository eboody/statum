# Statum Audit Playbook

Use this playbook when you want an agent to scan an existing Rust codebase for
good Statum refactor opportunities.

## Audit Workflow

1. Inventory obvious staged or lifecycle-heavy code.
2. Apply the fit rule strictly.
3. Rank only the best candidates.
4. Reject false positives explicitly instead of hand-waving.
5. If asked, implement the single best high-confidence fit after ranking.

## Lead Rule

Don't start from "does this thing have states?"

Start from this instead:

- A type is a good Statum or typestate candidate when the phase of the value
  should change what methods are legally available on that value.
- If you pressed `.` before and after a transition, you should want to see a
  meaningfully different method surface.

Don't flag code just because it has multiple internal steps.

Don't flag:

- plain linear orchestration where the intermediate values are private locals
- builders unless construction order itself is a meaningful invariant
- wrappers that mostly narrate a story without removing illegal calls from the
  API surface

## Search Pass

Start with a quick search sweep. Adjust the root paths for your repo:

```bash
rg -n "enum .*State|enum .*Status|state:|status:|phase:|stage:" src crates apps
rg -n "match .*status|match .*state|if .*status|if .*state|InvalidState|cannot .* while|only .* can" src crates apps
rg -n "approve|review|publish|activate|deactivate|retry|cancel|archive|ship|rollback|lease|handshake" src crates apps
rg -n "event log|projection|rehydrat|rebuild|snapshot|append-only" src crates apps
```

Then inspect a few high-signal files manually before proposing anything.

## What To Look For

Look for candidates where one or more are true:

- the same struct or value goes through named phases and different phases
  should expose different methods
- callers could misuse methods by calling them in the wrong order
- intermediate states escape a function or module and are interacted with
  directly
- the code uses stage structs, enums, manual comments, or helper naming to
  simulate legal next steps
- phase-specific data is only valid in some states and is awkwardly modeled
  with `Option`, booleans, or defensive checks
- branching legal paths would read better as distinct typed transitions

Be strict. The goal is the best fits, not every staged function in the repo.

## What the Agent Should Produce

Rank the results best-first. For each candidate, require this output shape:

```text
## Candidate: <symbol name>
Path: <file path>
Confidence: high | medium | low

1. Why should the method surface differ by phase?
2. Why is this better than a plain private function with locals?
3. Should this become:
   - a full Statum machine
   - a smaller typestate builder or surface
   - left alone
4. What is the smallest useful refactor slice?
```

The audit must also include:

- false positives considered and rejected
- why each rejected case is better left as plain local orchestration or runtime
  checks
- if there is one strongest candidate, a sketch of target states and transition
  methods

Don't implement anything unless the user explicitly asks for that follow-on.

## Bad-Fit Example

Don't recommend Statum for something like a search filter or dashboard view
model that has many toggles but no meaningful protocol ordering. That is normal
data modeling, not a staged workflow.

Another common false positive is a function with several private locals such as
`parse -> validate -> resolve -> render` where the intermediate values never
escape and the public method surface does not change. That may read as a story,
but it is not automatically a typestate surface.

## Optional Follow-On: Rank Then Implement One

If the user wants the audit to go straight into one refactor:

1. Rank the candidates first.
2. Pick the single best high-confidence fit.
3. Keep the refactor scoped.
4. Run tests for the affected crates.
5. Close out with a short CRIMEE summary and explain why this candidate was
   worth modeling with typestate.

## Statum References for the Audit

- [../../README.md](../../README.md)
- [../typestate-builder-design-playbook.md](../typestate-builder-design-playbook.md)
- [../patterns.md](../patterns.md)
- [../persistence-and-validators.md](../persistence-and-validators.md)
