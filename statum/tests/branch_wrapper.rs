#![allow(dead_code)]

use statum::{machine, state, transition, Branch, MachineIntrospection, MachineTransitionRecorder};

#[state]
enum BranchState {
    Draft,
    Accepted,
    Rejected,
}

#[machine]
struct BranchFlow<BranchState> {}

#[transition]
impl BranchFlow<Draft> {
    fn decide(self, accept: bool) -> Branch<BranchFlow<Accepted>, BranchFlow<Rejected>> {
        if accept {
            Branch::First(self.transition())
        } else {
            Branch::Second(self.transition())
        }
    }
}

#[test]
fn branch_wrapper_graph_exposes_both_targets() {
    let graph = <BranchFlow<Draft> as MachineIntrospection>::GRAPH;
    let decide = graph
        .transition_from_method(branch_flow::StateId::Draft, "decide")
        .unwrap();

    assert_eq!(
        graph.legal_targets(decide.id).unwrap(),
        &[
            branch_flow::StateId::Accepted,
            branch_flow::StateId::Rejected
        ]
    );
}

#[test]
fn branch_wrapper_runtime_join_records_concrete_target() {
    let accepted = BranchFlow::<Draft>::try_record_transition_to::<BranchFlow<Accepted>>(
        BranchFlow::<Draft>::DECIDE,
    )
    .unwrap();
    assert_eq!(accepted.chosen, branch_flow::StateId::Accepted);

    let rejected = BranchFlow::<Draft>::try_record_transition_to::<BranchFlow<Rejected>>(
        BranchFlow::<Draft>::DECIDE,
    )
    .unwrap();
    assert_eq!(rejected.chosen, branch_flow::StateId::Rejected);
}
