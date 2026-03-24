#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition};


#[state]
enum ProcessState {
    Init,
    NextState,
    OtherState,
    Finished,
}

#[machine]
struct ProcessMachine<ProcessState> {
    id: u64,
}

enum Decision {
    Next(ProcessMachine<NextState>),
    Other(ProcessMachine<OtherState>),
}

#[transition]
impl ProcessMachine<Init> {
    fn decide(self, event: u8) -> Decision {
        if event == 0 {
            Decision::Next(self.transition())
        } else {
            Decision::Other(self.transition())
        }
    }
}
