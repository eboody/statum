use crate::{Error, MachineIntrospection};
use statum_macros::{machine, state, transition, validators};

#[state]
pub enum GetUserState {
    Loading,
    Found,
    Missing,
}

#[machine]
pub struct GetUserFlow<GetUserState> {
    id: u64,
}

pub struct PersistedUser {
    found: bool,
}

#[transition]
impl GetUserFlow<Loading> {
    fn found(self) -> GetUserFlow<Found> {
        self.transition()
    }

    fn missing(self) -> GetUserFlow<Missing> {
        self.transition()
    }
}

#[validators(GetUserFlow)]
impl PersistedUser {
    fn is_loading(&self) -> Result<(), Error> {
        Err(Error::InvalidState)
    }

    fn is_found(&self) -> Result<(), Error> {
        if self.found {
            Ok(())
        } else {
            Err(Error::InvalidState)
        }
    }

    fn is_missing(&self) -> Result<(), Error> {
        if self.found {
            Err(Error::InvalidState)
        } else {
            Ok(())
        }
    }
}

pub fn assert_flow() {
    let found = GetUserFlow::<Loading>::builder().id(7).build().found();
    let missing = GetUserFlow::<Loading>::builder().id(9).build().missing();

    assert_eq!(found.id, 7);
    assert_eq!(missing.id, 9);

    let graph = <GetUserFlow<Loading> as MachineIntrospection>::GRAPH;
    assert!(
        graph
            .transition_from_method(self::get_user_flow::StateId::Loading, "found")
            .is_some()
    );
    assert!(
        graph
            .transition_from_method(self::get_user_flow::StateId::Loading, "missing")
            .is_some()
    );

    let rebuilt: self::get_user_flow::SomeState = PersistedUser { found: true }
        .into_machine()
        .id(11)
        .build()
        .unwrap();

    match rebuilt {
        self::get_user_flow::SomeState::Found(machine) => assert_eq!(machine.id, 11),
        _ => panic!("expected found state"),
    }
}
