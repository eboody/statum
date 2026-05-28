use statum::{
    MachineIntrospection, MachineStateIdentity, MachineTransitionRecorder, machine, state,
    transition,
};

#[state]
enum FlowState {
    Fetched,
    Accepted,
    Rejected,
}

#[machine]
struct Flow<FlowState> {
    request_id: u64,
}

#[transition]
impl Flow<Fetched> {
    fn validate(self, accept: bool) -> ::core::result::Result<Flow<Accepted>, Flow<Rejected>> {
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

pub fn run() -> Result<(), statum::Error> {
    let graph = <Flow<Fetched> as MachineIntrospection>::GRAPH;

    let validate = graph
        .transition_from_method(flow::StateId::Fetched, "validate")
        .ok_or(statum::Error::InvalidState)?;
    assert_eq!(validate.id, Flow::<Fetched>::VALIDATE);
    assert_eq!(
        graph
            .legal_targets(validate.id)
            .ok_or(statum::Error::InvalidState)?,
        &[flow::StateId::Accepted, flow::StateId::Rejected]
    );

    let fetched = Flow::<Fetched>::builder().request_id(7).build();
    let accepted = match fetched.validate(true) {
        Ok(accepted) => accepted,
        Err(_) => return Err(statum::Error::InvalidState),
    };
    assert_eq!(accepted.request_id, 7);

    let event = <Flow<Fetched> as MachineTransitionRecorder>::try_record_transition_to::<
        Flow<Accepted>,
    >(Flow::<Fetched>::VALIDATE)
    .ok_or(statum::Error::InvalidState)?;

    let transition = event
        .transition_in(graph)
        .ok_or(statum::Error::InvalidState)?;
    assert_eq!(transition.method_name, "validate");
    assert_eq!(
        transition.from,
        <Flow<Fetched> as MachineStateIdentity>::STATE_ID
    );
    assert_eq!(event.chosen, flow::StateId::Accepted);
    Ok(())
}
