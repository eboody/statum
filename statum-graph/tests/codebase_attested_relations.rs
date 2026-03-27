#![allow(dead_code)]

use statum_graph::{
    CodebaseDoc, CodebaseRelationBasis, CodebaseRelationKind, CodebaseRelationSource,
};

mod payment {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Authorized,
        Captured,
    }

    #[machine]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Authorized> {
        fn capture(self) -> Machine<Captured> {
            self.transition()
        }
    }
}

mod fulfillment {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        ReadyToShip,
        Shipping,
    }

    #[machine]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<ReadyToShip> {
        fn start_shipping(
            self,
            #[via(crate::payment::machine::via::Capture)] payment: crate::payment::Machine<
                crate::payment::Captured,
            >,
        ) -> Machine<Shipping> {
            let _ = payment;
            self.transition()
        }
    }
}

#[test]
fn linked_codebase_exports_attested_via_relations() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");

    let payment = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("payment::Machine"))
        .expect("payment machine");
    let fulfillment = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("fulfillment::Machine"))
        .expect("fulfillment machine");
    let captured = payment
        .states
        .iter()
        .find(|state| state.rust_name == "Captured")
        .expect("payment captured state");
    let authorized = payment
        .states
        .iter()
        .find(|state| state.rust_name == "Authorized")
        .expect("payment authorized state");
    let payment_capture = payment
        .transitions
        .iter()
        .find(|transition| transition.method_name == "capture")
        .expect("payment capture transition");
    let fulfillment_start = fulfillment
        .transitions
        .iter()
        .find(|transition| transition.method_name == "start_shipping")
        .expect("fulfillment start_shipping transition");

    assert_eq!(doc.relations().len(), 2);

    let direct_relation = doc
        .relations()
        .iter()
        .find(|relation| {
            relation.kind == CodebaseRelationKind::TransitionParam
                && relation.basis == CodebaseRelationBasis::DirectTypeSyntax
                && matches!(
                    relation.source,
                    CodebaseRelationSource::TransitionParam {
                        machine,
                        transition,
                        param_index: 0,
                        param_name: Some("payment"),
                    } if machine == fulfillment.index && transition == fulfillment_start.index
                )
        })
        .expect("direct payment relation");
    assert_eq!(direct_relation.target_machine, payment.index);
    assert_eq!(direct_relation.target_state, captured.index);
    assert_eq!(direct_relation.attested_via, None);

    let via_relation = doc
        .relations()
        .iter()
        .find(|relation| {
            relation.kind == CodebaseRelationKind::TransitionParam
                && relation.basis == CodebaseRelationBasis::ViaDeclaration
                && matches!(
                    relation.source,
                    CodebaseRelationSource::TransitionParam {
                        machine,
                        transition,
                        param_index: 0,
                        param_name: Some("payment"),
                    } if machine == fulfillment.index && transition == fulfillment_start.index
                )
        })
        .expect("attested payment relation");
    assert_eq!(via_relation.target_machine, payment.index);
    assert_eq!(via_relation.target_state, captured.index);

    let attested = via_relation.attested_via.expect("attested route");
    assert_eq!(attested.machine, payment.index);
    assert_eq!(attested.state, authorized.index);
    assert_eq!(attested.transition, payment_capture.index);
    assert!(attested.via_module_path.ends_with("payment::machine::via"));
    assert_eq!(attested.route_name, "Capture");

    let groups = doc.machine_relation_groups();
    let group = groups
        .iter()
        .find(|group| group.from_machine == fulfillment.index && group.to_machine == payment.index)
        .expect("fulfillment -> payment relation group");
    assert_eq!(group.relation_indices.len(), 2);
    assert_eq!(
        group
            .counts
            .iter()
            .map(|count| count.display_label())
            .collect::<Vec<_>>(),
        vec!["param".to_string(), "param [via]".to_string()]
    );

    let via_relation_index = group
        .relation_indices
        .iter()
        .copied()
        .find(|index| doc.relations()[*index].basis == CodebaseRelationBasis::ViaDeclaration)
        .expect("via relation index");
    let detail = doc
        .relation_detail(via_relation_index)
        .expect("attested relation detail");
    assert_eq!(detail.source_machine.index, fulfillment.index);
    assert_eq!(
        detail
            .source_transition
            .map(|transition| transition.method_name),
        Some("start_shipping")
    );
    assert_eq!(detail.target_machine.index, payment.index);
    assert_eq!(detail.target_state.rust_name, "Captured");
    assert_eq!(
        detail
            .attested_via_transition
            .map(|transition| transition.method_name),
        Some("capture")
    );
    assert_eq!(
        detail.attested_via_state.map(|state| state.rust_name),
        Some("Authorized")
    );
    assert_eq!(
        detail
            .attested_via_machine
            .map(|machine| machine.rust_type_path),
        Some(payment.rust_type_path)
    );
}
