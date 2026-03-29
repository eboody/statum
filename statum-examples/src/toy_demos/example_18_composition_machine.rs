use statum::{
    LinkedRelationBasis, LinkedRelationKind, LinkedRelationSource, LinkedRelationTarget,
    MachineRole, linked_machines, linked_relations, linked_via_routes, machine, state, transition,
};

mod review {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Pending,
        Approved,
    }

    /// Tracks the leaf review protocol for one document.
    #[machine]
    pub struct Machine<State> {
        document_id: u64,
    }

    #[transition]
    impl Machine<Pending> {
        /// Records that review accepted the current document revision.
        pub fn approve(self) -> Machine<Approved> {
            self.transition()
        }
    }
}

mod publication {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Ready,
        Published,
    }

    /// Tracks publication once a document is ready to ship.
    #[machine]
    pub struct Machine<State> {
        document_id: u64,
    }

    #[transition]
    impl Machine<Ready> {
        /// Publishes the approved document.
        pub fn publish(self) -> Machine<Published> {
            self.transition()
        }
    }
}

#[state]
enum DocumentFlowState {
    Draft,
    Reviewing(self::review::Machine<self::review::Pending>),
    Approved(self::review::Machine<self::review::Approved>),
    Published,
}

/// Orchestrates the document story across review and publication.
#[machine(role = composition)]
struct DocumentFlow<DocumentFlowState> {
    document_id: u64,
}

#[transition]
impl DocumentFlow<Draft> {
    /// Starts the document-level flow by entering review with a child machine.
    fn submit_for_review(
        self,
        review: self::review::Machine<self::review::Pending>,
    ) -> DocumentFlow<Reviewing> {
        self.transition_with(review)
    }
}

#[transition]
impl DocumentFlow<Reviewing> {
    /// Records that review reached the approved child-machine state.
    fn record_approval(
        self,
        review: self::review::Machine<self::review::Approved>,
    ) -> DocumentFlow<Approved> {
        self.transition_with(review)
    }
}

#[transition]
impl DocumentFlow<Approved> {
    /// Records the detached publication handoff on the composition machine.
    fn record_publication(
        self,
        #[via(self::publication::machine::via::Publish)]
        publication: self::publication::Machine<self::publication::Published>,
    ) -> DocumentFlow<Published> {
        let _ = publication;
        self.transition()
    }
}

pub fn run() {
    let document = DocumentFlow::<Draft>::builder().document_id(7).build();
    let review = review::Machine::<review::Pending>::builder()
        .document_id(7)
        .build();

    let document = document.submit_for_review(review);
    let review = review::Machine::<review::Pending>::builder()
        .document_id(7)
        .build()
        .approve();
    let document = document.record_approval(review);

    let published = publication::Machine::<publication::Ready>::builder()
        .document_id(7)
        .build()
        .publish_and_attest();
    let _document = document.from_publish(published).record_publication();

    let composition_machine = linked_machines()
        .iter()
        .find(|graph| {
            graph.machine.role == MachineRole::Composition
                && graph
                    .machine
                    .module_path
                    .ends_with("toy_demos::example_18_composition_machine")
                && graph.machine.rust_type_path.ends_with("DocumentFlow")
        })
        .expect("composition machine registration");
    assert_eq!(composition_machine.machine.role, MachineRole::Composition);

    let review_relation = linked_relations()
        .iter()
        .find(|relation| {
            relation
                .machine
                .module_path
                .ends_with("toy_demos::example_18_composition_machine")
                && relation.machine.rust_type_path.ends_with("DocumentFlow")
                && relation.kind == LinkedRelationKind::TransitionParam
                && relation.basis == LinkedRelationBasis::DirectTypeSyntax
                && matches!(
                    relation.source,
                    LinkedRelationSource::TransitionParam {
                        state: "Draft",
                        transition,
                        param_index: 0,
                        param_name: Some("review"),
                    } if transition == "submit_for_review"
                )
                && matches!(
                    relation.target,
                    LinkedRelationTarget::DirectMachine { state, .. } if state == "Pending"
                )
        })
        .expect("direct composition child relation");
    assert!(
        review_relation
            .machine
            .rust_type_path
            .ends_with("DocumentFlow")
    );

    let publish_direct = linked_relations()
        .iter()
        .find(|relation| {
            relation
                .machine
                .module_path
                .ends_with("toy_demos::example_18_composition_machine")
                && relation.machine.rust_type_path.ends_with("DocumentFlow")
                && relation.kind == LinkedRelationKind::TransitionParam
                && relation.basis == LinkedRelationBasis::DirectTypeSyntax
                && matches!(
                    relation.source,
                    LinkedRelationSource::TransitionParam {
                        state: "Approved",
                        transition,
                        param_index: 0,
                        param_name: Some("publication"),
                    } if transition == "record_publication"
                )
                && matches!(
                    relation.target,
                    LinkedRelationTarget::DirectMachine { state, .. } if state == "Published"
                )
        })
        .expect("direct publication relation");
    assert!(
        publish_direct
            .machine
            .rust_type_path
            .ends_with("DocumentFlow")
    );

    let publish_via = linked_relations()
        .iter()
        .find(|relation| {
            relation
                .machine
                .module_path
                .ends_with("toy_demos::example_18_composition_machine")
                && relation.machine.rust_type_path.ends_with("DocumentFlow")
                && relation.kind == LinkedRelationKind::TransitionParam
                && relation.basis == LinkedRelationBasis::ViaDeclaration
                && matches!(
                    relation.source,
                    LinkedRelationSource::TransitionParam {
                        state: "Approved",
                        transition,
                        param_index: 0,
                        param_name: Some("publication"),
                    } if transition == "record_publication"
                )
                && matches!(
                    relation.target,
                    LinkedRelationTarget::AttestedRoute {
                        state,
                        route_name,
                        ..
                    } if state == "Published" && route_name == "Publish"
                )
        })
        .expect("via publication relation");

    let LinkedRelationTarget::AttestedRoute { route_id, .. } = publish_via.target else {
        panic!("expected attested route target");
    };

    let producer = linked_via_routes()
        .iter()
        .find(|route| {
            route.route_id == route_id
                && route
                    .machine
                    .module_path
                    .ends_with("toy_demos::example_18_composition_machine::publication")
                && route.machine.rust_type_path.ends_with("publication::Machine")
                && route.transition == "publish"
                && route.source_state == "Ready"
                && route.target_state == "Published"
        })
        .expect("publication route producer");
    assert_eq!(producer.route_name, "Publish");
}
