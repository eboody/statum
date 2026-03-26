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
struct TaskId(u64);

#[machine_ref(crate::Task<crate::Running>)]
struct TaskKey {
    raw: u64,
}

fn assert_machine_ref<T: statum::MachineReference>() {}

fn main() {
    assert_machine_ref::<TaskId>();
    assert_machine_ref::<TaskKey>();

    let id = TaskId(7);
    let key = TaskKey { raw: 9 };
    let _ = id.0;
    let _ = key.raw;
}
