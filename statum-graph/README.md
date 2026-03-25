# statum-graph

`statum-graph` exports static machine topology directly from
`statum::MachineIntrospection::GRAPH`.

It is authoritative only for machine-local structure:

- machine identity
- states
- transition sites
- exact legal targets
- roots derivable from the static graph itself

It does not model runtime-selected branches, orchestration across multiple
machines, or consumer-owned explanation metadata.

## Install

```toml
[dependencies]
statum = "0.6.9"
statum-graph = "0.6.9"
```

## Example

```rust
use statum::{machine, state, transition};
use statum_graph::{render, MachineDoc};

#[state]
enum FlowState {
    Draft,
    Review,
    Accepted,
    Rejected,
}

#[machine]
struct Flow<FlowState> {}

#[transition]
impl Flow<Draft> {
    fn submit(self) -> Flow<Review> {
        self.transition()
    }
}

#[transition]
impl Flow<Review> {
    fn decide(
        self,
        accept: bool,
    ) -> ::core::result::Result<Flow<Accepted>, Flow<Rejected>> {
        if accept {
            Ok(self.accept())
        } else {
            Err(self.reject())
        }
    }

    fn accept(self) -> Flow<Accepted> {
        self.transition()
    }

    fn reject(self) -> Flow<Rejected> {
        self.transition()
    }
}

let doc = MachineDoc::from_machine::<Flow<Draft>>();
let mermaid = render::mermaid(&doc);

assert!(mermaid.contains("s1 -->|decide| s2"));
assert!(mermaid.contains("s1 -->|decide| s3"));
```
