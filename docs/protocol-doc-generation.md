# Protocol Docs Generation

Statum can render several review artifacts from one `StableGraphMetadata` value:

- Mermaid state diagrams for quick workflow shape review.
- Markdown transition tables that show allowed and forbidden edges.
- Narrative summaries for human reviewers.

For the flagship showcase, generate the combined artifact with:

```bash
cargo statum docs --machine axum-sqlite-review
```

The command gathers the supported machine metadata once, then renders every section from that same in-memory value. It does not re-scan Rust source, inspect macro expansion, run type checking, evaluate runtime policy, read validators, load persisted rows, or observe side effects while writing the docs artifact.

## Keeping Artifacts Current

Regenerate protocol docs whenever a workflow change touches states, transitions, labels, descriptions, metadata authority, or unsupported-case metadata:

```bash
mkdir -p docs/generated
cargo statum docs --machine axum-sqlite-review > docs/generated/axum-sqlite-review-protocol.md
```

Review the generated diff with the code change. Because Mermaid, transition-table, and narrative sections are rendered from the same `StableGraphMetadata` value, the regenerated artifact avoids hand-maintained disagreement. Individual sections can still change independently depending on which metadata fields changed, so reviewers should check whether the changed sections match the workflow update.

When committing a workflow change, include the regenerated docs artifact in the same change set as the Rust update. CI and reviewers can then compare the protocol code, generated metadata output, and docs artifact without chasing separately maintained diagrams or prose.

## Authority Boundary

Generated protocol docs describe the `StableGraphMetadata` observation point recorded in the artifact. Today that is `cfg_pruned_macro_input` for supported macro-input transition shapes. The generated docs do not claim complete Rust semantics, runtime authorization, validator behavior, persistence migrations, or external side effects.

If the metadata authority or unsupported-case list changes, review [Introspection authority boundaries](introspection-authority.md) before publishing regenerated docs.
