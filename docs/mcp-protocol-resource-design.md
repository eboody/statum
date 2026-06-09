# MCP Protocol Resource Design

This is a design note for a future MCP resource that exposes Statum protocol
metadata to coding agents. It is not an implementation plan for the current
crate graph.

## Scope

The resource should help agents inspect the protocol shape that Statum already
knows how to emit:

- machines
- states
- transition sites
- legal target states per transition site
- source-local presentation metadata, when present
- static provenance and limits of the emitted metadata

The first consumer is an agent connected to a local or project-scoped MCP server.
The server reads compiled or generated Statum metadata and returns a compact JSON
resource. The resource should be useful for questions like:

- "What states does this workflow expose?"
- "From `InReview`, what transitions are legal?"
- "Which branch targets can `validate` produce?"
- "What human label or description did the crate attach to this state?"
- "Can I trust this graph as strict transition-site metadata?"

The authority surface is static protocol metadata, not runtime business state.
The observation point is the same as Statum introspection: cfg-pruned attribute
macro input, directly readable transition signatures, explicit introspection
annotations, and supported presentation attributes for the active build. If the
server later reads a serialized artifact instead of Rust items directly, the
resource must say that its observation point is that artifact and include
artifact provenance.

## Resource Shape

Expose two resource families.

### Protocol index

URI:

```text
statum://protocols
```

MCP content:

```json
{
  "schema_version": "statum.protocol_index.v1",
  "crate": "my-service",
  "generated_at": "2026-05-29T00:00:00Z",
  "protocols": [
    {
      "id": "review_workflow::ReviewMachine",
      "uri": "statum://protocols/review_workflow/ReviewMachine",
      "label": "Review workflow",
      "state_count": 4,
      "transition_count": 6,
      "authority": {
        "claim": "strict_transition_site_metadata",
        "observation_point": "cfg_pruned_attribute_macro_input",
        "strict_introspection": true
      }
    }
  ]
}
```

The index is intentionally shallow. It lets an agent discover available machines
without loading every transition graph into context.

### Protocol detail

URI:

```text
statum://protocols/{module_path}/{machine_name}
```

MCP content:

```json
{
  "schema_version": "statum.protocol.v1",
  "id": "review_workflow::ReviewMachine",
  "machine": {
    "name": "ReviewMachine",
    "module_path": "review_workflow",
    "state_enum": "ReviewState",
    "label": "Review workflow",
    "description": "Typed lifecycle for editorial review."
  },
  "authority": {
    "claim": "strict_transition_site_metadata",
    "observation_point": "cfg_pruned_attribute_macro_input",
    "strict_introspection": true,
    "unsupported_cases": [
      "custom decision enums unless declared through an explicit introspection escape hatch",
      "wrapper aliases outside the supported canonical shapes",
      "nested cfg or cfg_attr on states, variant payload fields, or machine fields"
    ]
  },
  "states": [
    {
      "id": "Draft",
      "rust_type": "review_workflow::Draft",
      "label": "Draft",
      "description": "A review that can still be edited.",
      "data_shape": "unit"
    },
    {
      "id": "InReview",
      "rust_type": "review_workflow::InReview",
      "label": "In review",
      "description": "A review assigned to a reviewer.",
      "data_shape": "payload"
    }
  ],
  "transitions": [
    {
      "id": "ReviewMachine<Draft>::SUBMIT",
      "method": "submit",
      "source": "Draft",
      "targets": ["InReview"],
      "label": "Submit",
      "description": "Move a draft into review."
    },
    {
      "id": "ReviewMachine<InReview>::DECIDE",
      "method": "decide",
      "source": "InReview",
      "targets": ["Approved", "ChangesRequested"],
      "label": "Decide",
      "description": "Record the reviewer decision."
    }
  ],
  "runtime_join": {
    "static_transition_ids": true,
    "recorded_transition_type": "RecordedTransition",
    "notes": "Runtime events can reference these ids, but this resource does not expose event-log rows."
  },
  "presentation": {
    "source": "generated_present_attributes_or_manual_overlay",
    "typed_metadata": false
  },
  "provenance": {
    "crate_version": "0.8.10",
    "features": ["strict-introspection"],
    "target_triple": "x86_64-unknown-linux-gnu"
  }
}
```

Recommended transport behavior:

- Return `application/json` text content so generic MCP clients can display it.
- Keep every string stable and deterministic across builds when the protocol has
  not changed.
- Keep the resource read-only. Mutations belong in ordinary application tools,
  not protocol metadata resources.
- Prefer compact ids and arrays over prose-heavy explanations. Agents can ask
  for a separate human docs resource later if they need narrative text.

## Core Dependency Boundary

Do not add the MCP SDK, a JSON-RPC server, Tokio, or HTTP transport dependencies
to `statum-core`.

The dependency boundary should stay one of these shapes:

1. A separate `statum-mcp` adapter crate that depends on Statum metadata types and
   the MCP SDK.
2. A project-local MCP server in examples or downstream applications that reads a
   generated metadata artifact.
3. A small serializer feature that emits JSON-compatible data from existing
   descriptors, with the actual MCP server living outside the core crates.

`statum-core` may expose reusable descriptor data structures and optional
serialization support, but it should not own server lifecycle, transports,
credentials, agent policy, or resource routing.

## Non-Goals

This design does not attempt to:

- expose live workflow instances, persisted rows, event logs, leases, or user
  data;
- execute transitions or validators through MCP;
- replace Rustdoc, README prose, or generated human workflow guides;
- guarantee exhaustiveness beyond the active build and the supported
  introspection shapes;
- inspect arbitrary Rust control flow or transition method bodies;
- make relaxed introspection claims sound strict;
- couple Statum's core crates to a specific MCP SDK or async runtime;
- prescribe authorization for a hosted MCP server.

## Open Implementation Questions

- Should protocol detail resources be generated at compile time, discovered from
  `linkme` inventory at process startup, or loaded from a checked-in artifact?
- Should typed presentation metadata be serialized as opaque JSON, as named Rust
  type references, or omitted until a stable schema exists?
- Should URIs include crate version or target triple when the same workspace can
  expose multiple cfg-specific graphs?
- Should runtime transition records live under a separate URI family such as
  `statum://runs/{run_id}` if a downstream application wants event-log context?

The safest first version is an adapter outside `statum-core` that serves a
read-only JSON view of existing strict introspection descriptors. That proves the
agent workflow without adding a heavy runtime dependency to the core crates.
