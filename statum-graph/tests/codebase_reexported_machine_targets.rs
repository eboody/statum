#![allow(dead_code)]

use statum_graph::{CodebaseDoc, CodebaseRelationBasis, CodebaseRelationSource};

mod outbound_release {
    pub use machine::{Flow, Released};
    pub use machine::flow::via;

    mod machine {
        use statum::{machine, state, transition};

        #[state]
        pub enum State {
            Ready,
            Released,
        }

        #[machine]
        pub struct Flow<State> {}

        #[transition]
        impl Flow<Ready> {
            pub fn release(self) -> Flow<Released> {
                self.transition()
            }
        }
    }
}

mod broker {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Declared,
        AwaitingInbound,
    }

    #[machine]
    pub struct Flow<State> {}

    #[transition]
    impl Flow<Declared> {
        pub fn await_inbound(
            self,
            #[via(crate::outbound_release::via::Release)]
            release: crate::outbound_release::Flow<crate::outbound_release::Released>,
        ) -> Flow<AwaitingInbound> {
            let _ = release;
            self.transition()
        }
    }
}

#[test]
fn linked_codebase_resolves_reexported_machine_targets_exactly() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");

    let broker = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("broker::Flow"))
        .expect("broker flow");
    let outbound_release = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("outbound_release::machine::Flow"))
        .expect("outbound release flow");
    let released = outbound_release
        .states
        .iter()
        .find(|state| state.rust_name == "Released")
        .expect("released state");
    let await_inbound = broker
        .transitions
        .iter()
        .find(|transition| transition.method_name == "await_inbound")
        .expect("await_inbound transition");

    let relations = doc
        .relations()
        .iter()
        .filter(|relation| {
            matches!(
                relation.source,
                CodebaseRelationSource::TransitionParam {
                    machine,
                    transition,
                    param_name: Some("release"),
                    ..
                } if machine == broker.index && transition == await_inbound.index
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(relations.len(), 2);

    let direct = relations
        .iter()
        .find(|relation| relation.basis == CodebaseRelationBasis::DirectTypeSyntax)
        .expect("direct relation");
    assert_eq!(direct.target_machine, outbound_release.index);
    assert_eq!(direct.target_state, released.index);
    assert_eq!(direct.attested_via, None);

    let via = relations
        .iter()
        .find(|relation| relation.basis == CodebaseRelationBasis::ViaDeclaration)
        .expect("via relation");
    assert_eq!(via.target_machine, outbound_release.index);
    assert_eq!(via.target_state, released.index);
    let attested = via.attested_via.as_ref().expect("attested route");
    assert!(attested.via_module_path.ends_with("outbound_release::via"));
    assert_eq!(attested.route_name, "Release");
    assert_eq!(attested.producers.len(), 1);
}
