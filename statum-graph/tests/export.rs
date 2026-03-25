#![allow(dead_code)]

use statum_graph::{render, MachineDoc};

mod linear {
    use statum::{machine, state, transition};

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ReviewPayload {
        pub reviewer: &'static str,
    }

    #[state]
    pub enum State {
        Draft,
        Review(ReviewPayload),
        Published,
    }

    #[machine]
    pub struct Flow<State> {}

    #[transition]
    impl Flow<Draft> {
        fn submit(self) -> Flow<Review> {
            self.transition_with(ReviewPayload { reviewer: "amy" })
        }
    }

    #[transition]
    impl Flow<Review> {
        fn publish(self) -> Flow<Published> {
            self.transition()
        }
    }
}

mod branching {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Draft,
        Review,
        Accepted,
        Rejected,
        Archived,
    }

    #[machine]
    pub struct Flow<State> {}

    #[transition]
    impl Flow<Draft> {
        fn submit(self) -> Flow<Review> {
            self.transition()
        }
    }

    #[transition]
    impl Flow<Review> {
        fn maybe_decide(
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

    #[transition]
    impl Flow<Accepted> {
        fn archive(self) -> Flow<Archived> {
            self.transition()
        }
    }

    #[transition]
    impl Flow<Rejected> {
        fn archive(self) -> Flow<Archived> {
            self.transition()
        }
    }
}

mod multi_root {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        First,
        Second,
        Finished,
    }

    #[machine]
    pub struct Flow<State> {}

    #[transition]
    impl Flow<First> {
        fn finish(self) -> Flow<Finished> {
            self.transition()
        }
    }
}

mod no_root {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Draft,
        Review,
        Rejected,
    }

    #[machine]
    pub struct Flow<State> {}

    #[transition]
    impl Flow<Draft> {
        fn submit(self) -> Flow<Review> {
            self.transition()
        }
    }

    #[transition]
    impl Flow<Review> {
        fn reject(self) -> Flow<Rejected> {
            self.transition()
        }
    }

    #[transition]
    impl Flow<Rejected> {
        fn rework(self) -> Flow<Draft> {
            self.transition()
        }
    }
}

mod macro_generated {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Start,
        Enabled,
        MacroTarget,
    }

    #[machine]
    pub struct Flow<State> {}

    #[transition]
    impl Flow<Start> {
        fn enable(self) -> Flow<Enabled> {
            self.transition()
        }
    }

    macro_rules! generated_transitions {
        () => {
            #[transition]
            impl Flow<Enabled> {
                fn via_macro(self) -> Flow<MacroTarget> {
                    self.transition()
                }
            }
        };
    }

    generated_transitions!();
}

#[test]
fn exports_linear_machine_topology_from_graph() {
    let doc = MachineDoc::from_machine::<linear::Flow<linear::Draft>>();

    assert_eq!(doc.machine.rust_type_path, "export::linear::Flow");
    assert_eq!(
        doc.states
            .iter()
            .map(|state| (
                state.descriptor.rust_name,
                state.descriptor.has_data,
                state.is_root
            ))
            .collect::<Vec<_>>(),
        vec![
            ("Draft", false, true),
            ("Review", true, false),
            ("Published", false, false),
        ]
    );
    assert_eq!(
        doc.edges
            .iter()
            .map(|edge| edge.descriptor.method_name)
            .collect::<Vec<_>>(),
        vec!["submit", "publish"]
    );
}

#[test]
fn preserves_exact_branch_targets_and_sorts_edges_stably() {
    let doc = MachineDoc::from_machine::<branching::Flow<branching::Review>>();

    assert_eq!(
        doc.edges
            .iter()
            .map(|edge| edge.descriptor.method_name)
            .collect::<Vec<_>>(),
        vec![
            "submit",
            "accept",
            "maybe_decide",
            "reject",
            "archive",
            "archive"
        ]
    );

    let maybe_decide = doc
        .edges
        .iter()
        .find(|edge| edge.descriptor.method_name == "maybe_decide")
        .expect("branching transition");
    assert_eq!(
        maybe_decide
            .descriptor
            .to
            .iter()
            .map(|state| doc.state(*state).unwrap().descriptor.rust_name)
            .collect::<Vec<_>>(),
        vec!["Accepted", "Rejected"]
    );
}

#[test]
fn derives_multiple_roots_and_zero_roots_from_topology() {
    let multi_root = MachineDoc::from_machine::<multi_root::Flow<multi_root::First>>();
    assert_eq!(
        multi_root
            .roots()
            .map(|state| state.descriptor.rust_name)
            .collect::<Vec<_>>(),
        vec!["First", "Second"]
    );

    let no_root = MachineDoc::from_machine::<no_root::Flow<no_root::Draft>>();
    assert_eq!(no_root.roots().count(), 0);
}

#[test]
fn mermaid_snapshot_is_stable_for_reconverging_graphs() {
    let doc = MachineDoc::from_machine::<branching::Flow<branching::Draft>>();
    insta::assert_snapshot!("branching_flow_mermaid", render::mermaid(&doc));
}

#[test]
fn mermaid_renders_one_edge_per_legal_target() {
    let doc = MachineDoc::from_machine::<branching::Flow<branching::Draft>>();
    let mermaid = render::mermaid(&doc);

    assert_eq!(mermaid.matches("-->|maybe_decide|").count(), 2);
    assert!(mermaid.contains("s1 -->|maybe_decide| s2"));
    assert!(mermaid.contains("s1 -->|maybe_decide| s3"));
}

#[test]
fn exports_macro_generated_transition_sites() {
    let doc = MachineDoc::from_machine::<macro_generated::Flow<macro_generated::Enabled>>();

    assert_eq!(
        doc.edges
            .iter()
            .map(|edge| edge.descriptor.method_name)
            .collect::<Vec<_>>(),
        vec!["enable", "via_macro"]
    );
}
