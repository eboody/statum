#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    Rejection, StateDescriptor, StateMarker, TransitionDescriptor, UnitState, Validation,
};

use statum_macros::{machine, state, validators};

type ValidatorCheck<T> = core::result::Result<T, Error>;
type ValidatorDiagnostic<T> = statum::Validation<T>;

#[state]
enum TaskState {
    Draft,
    Done(Completion),
}

struct Completion {
    count: u8,
}

#[machine]
struct TaskMachine<TaskState> {
    name: String,
}

struct DbRow {
    status: &'static str,
}

#[validators(TaskMachine)]
impl DbRow {
    fn is_draft(&self) -> ValidatorCheck<()> {
        let _ = name;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_done(&self) -> ValidatorDiagnostic<Completion> {
        let _ = name;
        if self.status == "done" {
            Ok(Completion { count: 1 })
        } else {
            Err(statum::Rejection::new("not done"))
        }
    }
}

fn main() {
    let row = DbRow { status: "done" };
    let machine = row.into_machine().name("todo".to_string()).build().unwrap();
    let _ = machine;
}
