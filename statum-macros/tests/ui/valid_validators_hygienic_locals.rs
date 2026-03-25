#![allow(unused_imports)]
extern crate self as statum;
pub use statum_macros::__statum_emit_validator_methods_impl;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, validators};

#[state]
enum WorkflowState {
    Draft,
    Review(String),
    Done,
}

#[machine]
struct Workflow<WorkflowState> {
    persisted: &'static str,
    attempts: i64,
    data: &'static str,
    items: usize,
    fields: bool,
}

struct Row {
    status: &'static str,
    reviewer: Option<&'static str>,
}

#[validators(Workflow)]
impl Row {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        let _ = (persisted, attempts, data, items, fields);
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_review(&self) -> Result<String, statum_core::Error> {
        let _ = (persisted, attempts, data, items, fields);
        if self.status == "review" {
            Ok(self.reviewer.expect("reviewer").to_owned())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_done(&self) -> Result<(), statum_core::Error> {
        let _ = (persisted, attempts, data, items, fields);
        if self.status == "done" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

fn main() {
    use workflow::IntoMachinesExt as _;

    let rebuilt = Row {
        status: "review",
        reviewer: Some("alice"),
    }
    .into_machine()
    .persisted("single")
    .attempts(1)
    .data("payload")
    .items(2)
    .fields(true)
    .build()
    .unwrap();
    match rebuilt {
        workflow::SomeState::Review(machine) => {
            assert_eq!(machine.persisted, "single");
            assert_eq!(machine.attempts, 1);
            assert_eq!(machine.data, "payload");
            assert_eq!(machine.items, 2);
            assert!(machine.fields);
            assert_eq!(machine.state_data, "alice".to_owned());
        }
        _ => panic!("expected review state"),
    }

    let report = Row {
        status: "draft",
        reviewer: None,
    }
    .into_machine()
    .persisted("report")
    .attempts(2)
    .data("report-data")
    .items(3)
    .fields(false)
    .build_report();
    assert!(report.result.is_ok());
    assert!(!report.attempts.is_empty());

    let batch = vec![Row {
        status: "done",
        reviewer: None,
    }]
    .into_machines()
    .persisted("batch")
    .attempts(3)
    .data("batch-data")
    .items(4)
    .fields(true)
    .build();
    assert!(batch.into_iter().next().unwrap().is_ok());

    let batch_reports = vec![Row {
        status: "draft",
        reviewer: None,
    }]
    .into_machines()
    .persisted("batch-report")
    .attempts(4)
    .data("batch-report-data")
    .items(5)
    .fields(false)
    .build_reports();
    assert!(batch_reports.into_iter().next().unwrap().result.is_ok());

    let mapped = vec![Row {
        status: "review",
        reviewer: Some("bob"),
    }]
    .into_machines_by(|_| workflow::Fields {
        persisted: "mapped",
        attempts: 5,
        data: "mapped-data",
        items: 6,
        fields: true,
    })
    .build();
    assert!(mapped.into_iter().next().unwrap().is_ok());

    let mapped_reports = vec![Row {
        status: "done",
        reviewer: None,
    }]
    .into_machines_by(|_| workflow::Fields {
        persisted: "mapped-report",
        attempts: 6,
        data: "mapped-report-data",
        items: 7,
        fields: false,
    })
    .build_reports();
    assert!(mapped_reports.into_iter().next().unwrap().result.is_ok());
}
