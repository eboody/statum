extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineReference, MachineReferenceTarget,
    MachineStateIdentity, StateDescriptor, StateFamily, StateFamilyMember, StateMarker,
    TransitionDescriptor, TransitionInventory, UnitState,
};

use statum_macros::{machine, machine_ref, state};

#[state]
enum TaskState {
    Running,
}

#[machine]
struct Task<TaskState> {}

#[machine_ref(crate::Task<crate::Running>)]
type TaskId = u64;

fn main() {}
