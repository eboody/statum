use statum::{
    LinkedRelationBasis, LinkedRelationKind, LinkedRelationSource, LinkedRelationTarget,
    linked_relations, linked_via_routes, machine, state, transition,
};

#[state]
enum PaymentState {
    Authorized,
    Captured,
}

#[machine]
struct PaymentMachine<PaymentState> {}

#[transition]
impl PaymentMachine<Authorized> {
    fn capture(self) -> PaymentMachine<Captured> {
        self.transition()
    }
}

#[state]
enum FulfillmentState {
    ReadyToShip,
    Shipping,
}

#[machine]
struct FulfillmentMachine<FulfillmentState> {}

#[transition]
impl FulfillmentMachine<ReadyToShip> {
    fn start_shipping(
        self,
        // Use an explicit machine path so the plain parameter also contributes
        // a direct exact relation alongside the attested `#[via(...)]` one.
        #[via(self::payment_machine::via::Capture)]
        payment: crate::toy_demos::example_17_attested_composition::PaymentMachine<
            crate::toy_demos::example_17_attested_composition::Captured,
        >,
    ) -> FulfillmentMachine<Shipping> {
        let _ = payment;
        self.transition()
    }
}

pub fn run() {
    let plain_payment = PaymentMachine::<Authorized>::builder().build().capture();
    let plain_shipping = FulfillmentMachine::<ReadyToShip>::builder()
        .build()
        .start_shipping(plain_payment);
    let _ = plain_shipping;

    let captured = PaymentMachine::<Authorized>::builder()
        .build()
        .capture_and_attest();
    let shipping = FulfillmentMachine::<ReadyToShip>::builder()
        .build()
        .from_capture(captured)
        .start_shipping();
    let _ = shipping;

    let direct_relation = linked_relations()
        .iter()
        .find(|relation| {
            relation
                .machine
                .module_path
                .ends_with("toy_demos::17-attested-composition")
                && relation.kind == LinkedRelationKind::TransitionParam
                && relation.basis == LinkedRelationBasis::DirectTypeSyntax
                && matches!(
                    relation.source,
                    LinkedRelationSource::TransitionParam {
                        state: "ReadyToShip",
                        transition,
                        param_index: 0,
                        param_name: Some("payment"),
                    } if transition == "start_shipping"
                )
                && matches!(
                    relation.target,
                    LinkedRelationTarget::DirectMachine { state, .. } if state == "Captured"
                )
        })
        .expect("direct payment relation");
    assert!(
        direct_relation
            .machine
            .rust_type_path
            .ends_with("FulfillmentMachine")
    );

    let via_relation = linked_relations()
        .iter()
        .find(|relation| {
            relation
                .machine
                .module_path
                .ends_with("toy_demos::17-attested-composition")
                && relation.kind == LinkedRelationKind::TransitionParam
                && relation.basis == LinkedRelationBasis::ViaDeclaration
                && matches!(
                    relation.source,
                    LinkedRelationSource::TransitionParam {
                        state: "ReadyToShip",
                        transition,
                        param_index: 0,
                        param_name: Some("payment"),
                    } if transition == "start_shipping"
                )
                && matches!(
                    relation.target,
                    LinkedRelationTarget::AttestedRoute {
                        state,
                        route_name,
                        ..
                    } if state == "Captured" && route_name == "Capture"
                )
        })
        .expect("attested payment relation");

    let LinkedRelationTarget::AttestedRoute {
        via_module_path,
        route_id,
        route_name,
        ..
    } = via_relation.target
    else {
        panic!("expected attested route target");
    };
    assert_eq!(route_name, "Capture");
    assert!(via_module_path.ends_with("toy_demos::17-attested-composition::payment_machine::via"));

    let producer = linked_via_routes()
        .iter()
        .find(|route| {
            route.route_id == route_id
                && route
                    .machine
                    .module_path
                    .ends_with("toy_demos::17-attested-composition")
                && route.transition == "capture"
                && route.source_state == "Authorized"
                && route.target_state == "Captured"
        })
        .expect("attested producer route");
    assert_eq!(producer.source_state, "Authorized");
    assert_eq!(producer.transition, "capture");
    assert_eq!(producer.target_state, "Captured");
}
