#![allow(dead_code)]

use statum_graph::{CodebaseDoc, CodebaseRelationBasis, CodebaseRelationSource};

mod workflow {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Rejected,
        Consumed,
        Destroyed,
    }

    #[machine]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Rejected> {
        fn destroy(self) -> Machine<Destroyed> {
            self.transition()
        }
    }

    #[transition]
    impl Machine<Consumed> {
        fn destroy(self) -> Machine<Destroyed> {
            self.transition()
        }
    }
}

mod audit {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Pending,
        Closed,
    }

    #[machine]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Pending> {
        fn close(
            self,
            #[via(crate::workflow::machine::via::Destroy)] destroyed: crate::workflow::Machine<
                crate::workflow::Destroyed,
            >,
        ) -> Machine<Closed> {
            let _ = destroyed;
            self.transition()
        }
    }
}

#[test]
fn duplicate_attested_route_names_group_compatible_producers() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");

    let workflow = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("workflow::Machine"))
        .expect("workflow machine");
    let audit = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("audit::Machine"))
        .expect("audit machine");
    let rejected = workflow
        .states
        .iter()
        .find(|state| state.rust_name == "Rejected")
        .expect("rejected state");
    let rejected_label = if rejected.direct_construction_available {
        format!("{} [build]", rejected.display_label())
    } else {
        rejected.display_label().into_owned()
    };
    let consumed = workflow
        .states
        .iter()
        .find(|state| state.rust_name == "Consumed")
        .expect("consumed state");
    let consumed_label = if consumed.direct_construction_available {
        format!("{} [build]", consumed.display_label())
    } else {
        consumed.display_label().into_owned()
    };
    let destroyed = workflow
        .states
        .iter()
        .find(|state| state.rust_name == "Destroyed")
        .expect("destroyed state");
    let destroyed_label = if destroyed.direct_construction_available {
        format!("{} [build]", destroyed.display_label())
    } else {
        destroyed.display_label().into_owned()
    };
    let closed_transition = audit
        .transitions
        .iter()
        .find(|transition| transition.method_name == "close")
        .expect("close transition");

    let relation = doc
        .relations()
        .iter()
        .find(|relation| {
            relation.basis == CodebaseRelationBasis::ViaDeclaration
                && matches!(
                    relation.source,
                    CodebaseRelationSource::TransitionParam {
                        machine,
                        transition,
                        param_name: Some("destroyed"),
                        ..
                    } if machine == audit.index && transition == closed_transition.index
                )
        })
        .expect("attested relation");
    let attested = relation.attested_via.as_ref().expect("attested route");
    assert_eq!(attested.route_name, "Destroy");
    assert_eq!(attested.producers.len(), 2);
    assert_eq!(
        attested
            .producers
            .iter()
            .map(|producer| producer.state)
            .collect::<Vec<_>>(),
        vec![rejected.index, consumed.index]
    );

    let detail = doc
        .relation_detail(relation.index)
        .expect("attested relation detail");
    assert_eq!(detail.attested_via_machine, None);
    assert_eq!(detail.attested_via_state, None);
    assert_eq!(detail.attested_via_transition, None);
    assert_eq!(detail.attested_via_producers.len(), 2);
    assert_eq!(
        detail
            .attested_via_producers
            .iter()
            .map(|producer| producer.state.rust_name)
            .collect::<Vec<_>>(),
        vec!["Rejected", "Consumed"]
    );
    assert_eq!(
        detail
            .attested_via_producers
            .iter()
            .map(|producer| producer.transition.method_name)
            .collect::<Vec<_>>(),
        vec!["destroy", "destroy"]
    );

    let mermaid = statum_graph::codebase::render::mermaid_relation_sequence(&doc, relation.index)
        .expect("multi-producer relation sequence");
    assert!(mermaid.contains("sequenceDiagram"));
    assert!(mermaid.contains(&format!("alt destroy from {}", rejected_label)));
    assert!(mermaid.contains(&format!("else destroy from {}", consumed_label)));
    assert!(mermaid.contains(&format!(
        "m{}->>m{}: {} via Destroy",
        workflow.index, audit.index, destroyed_label
    )));
}
