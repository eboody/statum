# Agent Maintainer Checklist

Use this checklist before changing a workflow, protocol, generated document, or
agent-facing instruction in this repo. It is meant to keep agent changes scoped,
testable, and honest about what evidence they used.

## Before Editing

1. Read the repository guidance first.
   - Start with `AGENTS.md`.
   - Read the graph map required by the repo guidance. If `graphify-out/GRAPH_REPORT.md`
     is unavailable, say so in the closeout and use the next available project map
     such as GitNexus context, docs indexes, or focused source inspection.
2. Locate the workflow boundary.
   - Name the state, protocol, job, or generated artifact being changed.
   - Identify whether the change affects user docs, agent prompts, macro output,
     telemetry labels, tests, or runtime behavior.
3. State the authority surface before broad claims.
   - Say what the change observes: raw source, parsed AST, cfg-pruned AST,
     expanded items, type-checked items, runtime values, persisted state, or
     handwritten docs.
   - If the observation point is weaker than the claim, narrow the claim or make
     unsupported cases fail closed.
4. Check existing examples and docs.
   - Prefer updating the existing tutorial, case study, or agent asset over adding
     a parallel explanation that can drift.
   - Keep the root `README.md` aligned when public API examples or positioning
     change.

## While Editing

1. Keep the workflow contract explicit.
   - Name legal states or phases.
   - Name allowed transitions.
   - Name rejected transitions and why they remain runtime-validated, typed, or
     unsupported.
2. Update tests with the behavior.
   - Add or adjust focused tests for any behavior change.
   - For macro UI tests, register new fixtures explicitly in
     `statum-macros/tests/macro_errors.rs` and refresh `.stderr` only after
     inspecting diagnostics by hand.
   - For docs-only changes, update links and examples instead of adding unrelated
     code tests.
3. Add adversarial authority cases when the change claims to extract or generate
   workflow truth.
   - Cover cases that differ by observation stage: `#[cfg]`, macro-generated
     items, `include!`, duplicate ids, hidden module boundaries, or persisted
     rows that cannot rebuild into a typed machine.
   - Prefer fail-closed diagnostics over guessed edges or silently missing states.
4. Avoid decorative prose.
   - Say what changed, what evidence supports it, and what remains outside scope.
   - Do not describe a generated table, diagram, or prompt as complete unless the
     implementation proves the same authority surface.

## Before Closing

1. Run the narrowest gate that proves the change.
   - Docs-only: run `bash scripts/check_readme_links.sh`; add
     `bash scripts/check_workspace_hygiene.sh` when adding, moving, or deleting
     files.
   - Rust behavior, macros, diagnostics pages, graph/protocol reports, or closeout
     tasks: run the focused tests first, then `bash scripts/check_ci_parity.sh`
     when practical.
   - If a narrower gate is sufficient, say why in the closeout.
2. Inspect the diff.
   - Confirm only intended files changed.
   - Do not overwrite unrelated workspace changes.
3. Close out with maintainer evidence.
   - Changed files.
   - Gates run and results.
   - Claimed authority surface.
   - Actual observation point.
   - Unsupported cases rejected or left open.
   - Adversarial tests added, or why none were needed for docs-only work.
