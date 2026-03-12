# Statum Audit Playbook

Use this playbook when you want an agent to scan an existing Rust codebase for
good Statum refactor opportunities.

## Audit Workflow

1. Inventory obvious lifecycle code.
2. Score each candidate.
3. Sketch the Statum shape for strong fits.
4. Reject weak fits explicitly instead of hand-waving.

## Search Pass

Start with a quick search sweep. Adjust the root paths for your repo:

```bash
rg -n "enum .*State|enum .*Status|state:|status:|phase:|stage:" src crates apps
rg -n "match .*status|match .*state|if .*status|if .*state|InvalidState|cannot .* while|only .* can" src crates apps
rg -n "approve|review|publish|activate|deactivate|retry|cancel|archive|ship|rollback|lease|handshake" src crates apps
rg -n "event log|projection|rehydrat|rebuild|snapshot|append-only" src crates apps
```

Then inspect a few high-signal files manually before proposing anything.

## Candidate Scoring

Score each candidate from `0` to `2` in these categories:

- finite phases
- expensive illegal edges
- phase-specific methods or data
- duplicated runtime guard logic
- lifecycle stability

Add `+1` if rebuild from rows, snapshots, or event logs is central to the
workflow.

Interpretation:

- `8-11`: strong Statum candidate
- `5-7`: maybe; only propose if the code pain is concrete
- `0-4`: poor fit; keep runtime validation

## What the Agent Should Produce

For each strong candidate, require this output shape:

```text
## Candidate: <entity name>
Fit: strong | maybe | poor

Evidence
- <file/symbol and why it matters>
- <file/symbol and why it matters>

Current pain
- <duplicated guards, invalid transitions, optional-field drift, rebuild noise>

Proposed Statum shape
- `#[state]` enum: <state list>
- `#[machine]` fields: <shared context>
- state data: <phase-only payloads>
- `#[transition]` blocks: <legal edges>
- `#[validators]` / `statum::projection`: <yes/no and why>

Migration slice
- <smallest first refactor worth doing>

Risk
- low | medium | high
- <what could make this expensive>
```

If a candidate is a poor fit, the agent should say why and stop there.

## Bad-Fit Example

Do not recommend Statum for something like a search filter or dashboard view
model that has many toggles but no meaningful protocol ordering. That is normal
data modeling, not a staged workflow.

## Statum References for the Audit

- [../../README.md](../../README.md)
- [../typestate-builder-design-playbook.md](../typestate-builder-design-playbook.md)
- [../patterns.md](../patterns.md)
- [../persistence-and-validators.md](../persistence-and-validators.md)
