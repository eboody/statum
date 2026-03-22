#![allow(dead_code)]

use statum::{machine, state, transition, MachineIntrospection};

#[state]
enum GeneratedState {
    Start,
    Enabled,
    MacroTarget,
    Included,
    Hidden,
}

#[machine]
struct GeneratedFlow<GeneratedState> {}

#[cfg(any())]
#[transition]
impl GeneratedFlow<Start> {
    fn cfg_impl_hidden(self) -> GeneratedFlow<Hidden> {
        self.transition()
    }
}

#[transition]
impl GeneratedFlow<Start> {
    fn enable(self) -> GeneratedFlow<Enabled> {
        self.transition()
    }

    #[cfg(any())]
    fn cfg_method_hidden(self) -> GeneratedFlow<Hidden> {
        self.transition()
    }
}

macro_rules! generated_transitions {
    () => {
        #[transition]
        impl GeneratedFlow<Enabled> {
            fn via_macro(self) -> GeneratedFlow<MacroTarget> {
                self.transition()
            }
        }
    };
}

generated_transitions!();

include!("support/generated_flow_include.rs");

#[test]
fn graph_respects_cfg_pruning_and_macro_generated_transitions() {
    let graph = <GeneratedFlow<Start> as MachineIntrospection>::GRAPH;

    let mut start_methods = graph
        .transitions_from(generated_flow::StateId::Start)
        .map(|transition| transition.method_name)
        .collect::<Vec<_>>();
    start_methods.sort_unstable();
    assert_eq!(start_methods, vec!["enable"]);
    assert!(graph
        .transition_from_method(generated_flow::StateId::Start, "cfg_impl_hidden")
        .is_none());
    assert!(graph
        .transition_from_method(generated_flow::StateId::Start, "cfg_method_hidden")
        .is_none());

    let via_macro = graph
        .transition_from_method(generated_flow::StateId::Enabled, "via_macro")
        .expect("macro-generated transition should be in GRAPH");
    assert_eq!(
        graph.legal_targets(via_macro.id).unwrap(),
        &[generated_flow::StateId::MacroTarget]
    );

    let via_include = graph
        .transition_from_method(generated_flow::StateId::MacroTarget, "via_include")
        .expect("include-generated transition should be in GRAPH");
    assert_eq!(
        graph.legal_targets(via_include.id).unwrap(),
        &[generated_flow::StateId::Included]
    );
}
