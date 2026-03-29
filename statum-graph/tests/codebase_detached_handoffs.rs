#![allow(dead_code)]

use statum_graph::{
    CodebaseDoc, CodebaseRelationBasis, CodebaseRelationKind, CodebaseRelationSemantic,
    CodebaseRelationSource,
};

const fn route_id(input: &str) -> u64 {
    let bytes = input.as_bytes();
    let mut hash = 0xcbf29ce484222325u64;
    let mut index = 0usize;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        index += 1;
    }
    hash
}

const PUBLISH_ROUTE_ID: u64 = route_id("Publish");

mod publish {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Draft,
        Published,
    }

    #[machine]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Draft> {
        pub fn publish(self) -> Machine<Published> {
            self.transition()
        }
    }

    pub use machine::via;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    id: u64,
}

impl From<publish::Machine<publish::Published>> for Receipt {
    fn from(_: publish::Machine<publish::Published>) -> Self {
        Self { id: 1 }
    }
}

mod workflow {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Draft,
        Holding(::statum::Attested<
            crate::Receipt,
            crate::publish::machine::via::Route<{ crate::PUBLISH_ROUTE_ID }>,
        >),
        Recorded(crate::Receipt),
    }

    #[machine(role = composition)]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Draft> {
        pub fn hold(
            self,
            receipt: ::statum::Attested<
                crate::Receipt,
                crate::publish::machine::via::Route<{ crate::PUBLISH_ROUTE_ID }>,
            >,
        ) -> Machine<Holding> {
            self.transition_with(receipt)
        }

        pub fn record(
            self,
            #[via(crate::publish::via::Publish)] receipt: crate::Receipt,
        ) -> Machine<Recorded> {
            self.transition_with(receipt)
        }
    }
}

#[test]
fn detached_attested_receipts_flow_through_generated_binders() {
    let receipt = publish::Machine::<publish::Draft>::builder()
        .build()
        .publish_and_attest()
        .map_inner(Receipt::from);

    let held = workflow::Machine::<workflow::Draft>::builder()
        .build()
        .hold(receipt.clone());
    let _recorded = workflow::Machine::<workflow::Draft>::builder()
        .build()
        .from_publish(receipt)
        .record();

    assert_eq!(held.state_data.as_ref().id, 1);
}

#[test]
fn linked_codebase_exports_detached_handoffs_as_composition_relations() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");

    let publish = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("publish::Machine"))
        .expect("publish machine");
    let workflow = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("workflow::Machine"))
        .expect("workflow machine");
    let published = publish
        .states
        .iter()
        .find(|state| state.rust_name == "Published")
        .expect("published state");
    let draft = publish
        .states
        .iter()
        .find(|state| state.rust_name == "Draft")
        .expect("publish draft state");
    let publish_transition = publish
        .transitions
        .iter()
        .find(|transition| transition.method_name == "publish")
        .expect("publish transition");
    let workflow_hold = workflow
        .transitions
        .iter()
        .find(|transition| transition.method_name == "hold")
        .expect("hold transition");
    let workflow_record = workflow
        .transitions
        .iter()
        .find(|transition| transition.method_name == "record")
        .expect("record transition");
    let holding = workflow
        .states
        .iter()
        .find(|state| state.rust_name == "Holding")
        .expect("holding state");

    let state_payload_relation = doc
        .relations()
        .iter()
        .find(|relation| {
            relation.kind == CodebaseRelationKind::StatePayload
                && relation.basis == CodebaseRelationBasis::AttestedTypeSyntax
                && matches!(
                    relation.source,
                    CodebaseRelationSource::StatePayload {
                        machine,
                        state,
                        field_name: None,
                    } if machine == workflow.index && state == holding.index
                )
        })
        .expect("state payload attested relation");
    assert_eq!(state_payload_relation.target_machine, publish.index);
    assert_eq!(state_payload_relation.target_state, published.index);
    assert_eq!(
        state_payload_relation.semantic,
        CodebaseRelationSemantic::CompositionDetachedHandoff
    );

    let hold_param_relation = doc
        .relations()
        .iter()
        .find(|relation| {
            relation.kind == CodebaseRelationKind::TransitionParam
                && relation.basis == CodebaseRelationBasis::AttestedTypeSyntax
                && matches!(
                    relation.source,
                    CodebaseRelationSource::TransitionParam {
                        machine,
                        transition,
                        param_index: 0,
                        param_name: Some("receipt"),
                    } if machine == workflow.index && transition == workflow_hold.index
                )
        })
        .expect("raw attested param relation");
    assert_eq!(hold_param_relation.target_machine, publish.index);
    assert_eq!(hold_param_relation.target_state, published.index);
    assert_eq!(
        hold_param_relation.semantic,
        CodebaseRelationSemantic::CompositionDetachedHandoff
    );

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
                        param_name: Some("receipt"),
                    } if machine == workflow.index && transition == workflow_record.index
                )
        })
        .expect("via relation");
    assert_eq!(via_relation.target_machine, publish.index);
    assert_eq!(via_relation.target_state, published.index);
    assert_eq!(
        via_relation.semantic,
        CodebaseRelationSemantic::CompositionDetachedHandoff
    );
    let attested = via_relation.attested_via.as_ref().expect("attested route");
    assert_eq!(attested.route_name, "Publish");
    assert_eq!(attested.producers.len(), 1);
    assert_eq!(attested.producers[0].machine, publish.index);
    assert_eq!(attested.producers[0].state, draft.index);
    assert_eq!(attested.producers[0].transition, publish_transition.index);

    let group = doc
        .machine_relation_groups()
        .into_iter()
        .find(|group| group.from_machine == workflow.index && group.to_machine == publish.index)
        .expect("workflow -> publish relation group");
    assert!(group.is_composition_owned());
    assert_eq!(
        group.counts
            .iter()
            .map(|count| count.display_label())
            .collect::<Vec<_>>(),
        vec![
            "payload [attested]".to_owned(),
            "param [attested]".to_owned(),
            "param [via]".to_owned(),
        ]
    );

    let detail = doc
        .relation_detail(via_relation.index)
        .expect("via relation detail");
    assert_eq!(detail.source_machine.index, workflow.index);
    assert_eq!(detail.target_machine.index, publish.index);
    assert_eq!(detail.target_state.rust_name, "Published");
    assert_eq!(
        detail
            .attested_via_machine
            .map(|machine| machine.rust_type_path),
        Some(publish.rust_type_path)
    );
    assert_eq!(
        detail.attested_via_state.map(|state| state.rust_name),
        Some("Draft")
    );
    assert_eq!(
        detail
            .attested_via_transition
            .map(|transition| transition.method_name),
        Some("publish")
    );
}
