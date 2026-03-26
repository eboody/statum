#![allow(dead_code)]

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use statum::{
    LinkedMachineGraph, LinkedStateDescriptor, LinkedTransitionDescriptor,
    LinkedTransitionInventory, LinkedValidatorEntryDescriptor, MachineDescriptor,
    StaticMachineLinkDescriptor,
};
use statum_graph::{codebase::render, CodebaseDoc};

mod task {
    use statum::{machine, state, transition, validators, Error};

    #[state]
    pub enum State {
        Idle,
        #[present(label = "Running")]
        Running,
        Done,
    }

    #[machine]
    #[present(label = "Task Machine")]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Idle> {
        #[present(label = "Start Task")]
        fn start(self) -> Machine<Running> {
            self.transition()
        }
    }

    #[transition]
    impl Machine<Running> {
        fn finish(self) -> Machine<Done> {
            self.transition()
        }
    }

    pub struct TaskRow {
        pub status: &'static str,
    }

    #[validators(Machine)]
    impl TaskRow {
        fn is_idle(&self) -> statum::Result<()> {
            if self.status == "idle" {
                Ok(())
            } else {
                Err(Error::InvalidState)
            }
        }

        fn is_running(&self) -> statum::Result<()> {
            if self.status == "running" {
                Ok(())
            } else {
                Err(Error::InvalidState)
            }
        }

        fn is_done(&self) -> statum::Result<()> {
            if self.status == "done" {
                Ok(())
            } else {
                Err(Error::InvalidState)
            }
        }
    }
}

mod workflow {
    use super::*;
    use statum::{machine, state, transition, validators, Error};

    #[state]
    pub enum State {
        Draft,
        #[present(label = "In Progress")]
        InProgress(task::Machine<task::Running>),
        Complete,
    }

    #[machine]
    #[present(label = "Workflow Machine")]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Draft> {
        #[present(label = "Start Workflow")]
        fn start(self, running_task: task::Machine<task::Running>) -> Machine<InProgress> {
            self.transition_with(running_task)
        }
    }

    #[transition]
    impl Machine<InProgress> {
        fn finish(self) -> Machine<Complete> {
            self.transition()
        }
    }

    pub struct WorkflowRow {
        pub status: &'static str,
    }

    #[validators(Machine)]
    impl WorkflowRow {
        fn is_draft(&self) -> statum::Result<()> {
            if self.status == "draft" {
                Ok(())
            } else {
                Err(Error::InvalidState)
            }
        }

        fn is_in_progress(&self) -> statum::Result<task::Machine<task::Running>> {
            if self.status == "in_progress" {
                Ok(task::Machine::<task::Running>::builder().build())
            } else {
                Err(Error::InvalidState)
            }
        }

        fn is_complete(&self) -> statum::Result<()> {
            if self.status == "complete" {
                Ok(())
            } else {
                Err(Error::InvalidState)
            }
        }
    }
}

mod named_holder {
    use super::*;
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Pending {
            child: task::Machine<task::Done>,
            note: &'static str,
        },
        Settled,
    }

    #[machine]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Pending> {
        fn settle(self) -> Machine<Settled> {
            self.transition()
        }
    }
}

mod detached {
    use statum::{machine, state};

    #[state]
    pub enum State {
        Alone,
    }

    #[machine]
    pub struct Machine<State> {}
}

#[test]
fn linked_codebase_doc_collects_machines_and_links() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");

    assert_eq!(doc.machines().len(), 4);
    assert_eq!(doc.links().len(), 2);

    let workflow = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("workflow::Machine"))
        .expect("workflow machine");
    assert_eq!(workflow.label, Some("Workflow Machine"));
    assert_eq!(
        workflow
            .states
            .iter()
            .find(|state| state.rust_name == "InProgress")
            .map(|state| state.label),
        Some(Some("In Progress"))
    );
    assert_eq!(workflow.validator_entries.len(), 1);
    assert_eq!(
        workflow.validator_entries[0].source_type_display,
        "WorkflowRow"
    );
    assert_eq!(workflow.validator_entries[0].target_states, vec![0, 1, 2]);

    let workflow_link = doc
        .links()
        .iter()
        .find(|link| {
            doc.machine(link.from_machine)
                .map(|machine| machine.rust_type_path.ends_with("workflow::Machine"))
                .unwrap_or(false)
        })
        .expect("workflow link");
    assert_eq!(workflow_link.field_name, None);

    let named_link = doc
        .links()
        .iter()
        .find(|link| link.field_name == Some("child"))
        .expect("named child link");
    let target_machine = doc
        .machine(named_link.to_machine)
        .expect("named link target machine");
    let target_state = target_machine
        .state(named_link.to_state)
        .expect("named link target state");
    assert!(target_machine.rust_type_path.ends_with("task::Machine"));
    assert_eq!(target_state.rust_name, "Done");
    assert_eq!(target_machine.validator_entries.len(), 1);
    assert_eq!(
        target_machine.validator_entries[0].display_label().as_ref(),
        "TaskRow::into_machine()"
    );
}

#[test]
fn linked_codebase_renderers_are_stable() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");

    insta::assert_snapshot!("linked_codebase_mermaid", render::mermaid(&doc));
    insta::assert_snapshot!("linked_codebase_dot", render::dot(&doc));
    insta::assert_snapshot!("linked_codebase_plantuml", render::plantuml(&doc));
    insta::assert_snapshot!("linked_codebase_json", render::json(&doc));
}

#[test]
fn linked_codebase_writes_all_formats() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");
    let dir = tempfile::tempdir().expect("temp dir");

    let paths = render::write_all_to_dir(&doc, dir.path().join("nested"), "codebase")
        .expect("write linked codebase bundle");

    let file_names = paths
        .iter()
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        file_names,
        vec![
            "codebase.mmd",
            "codebase.dot",
            "codebase.puml",
            "codebase.json",
        ]
    );

    let mermaid_path = dir.path().join("nested").join("codebase.mmd");
    assert!(mermaid_path.exists());
    let mermaid = fs::read_to_string(mermaid_path).expect("mermaid file");
    assert!(mermaid.contains("Workflow Machine"));
    assert!(mermaid.contains("Task Machine"));
}

#[test]
fn linked_codebase_write_all_rejects_path_like_stem() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");
    let dir = tempfile::tempdir().expect("temp dir");
    let bundle_dir = dir.path().join("nested");
    let outside = dir.path().join("escape.mmd");
    let stem = Path::new("..").join("escape");

    let error = render::write_all_to_dir(&doc, &bundle_dir, stem.to_str().expect("utf-8 stem"))
        .expect_err("path-like stem should be rejected");

    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert!(!bundle_dir.exists());
    assert!(!outside.exists());
}

#[test]
fn malformed_inventory_rejects_missing_transition_source_before_sort() {
    fn transitions() -> &'static [LinkedTransitionDescriptor] {
        &TRANSITIONS
    }

    static STATES: [LinkedStateDescriptor; 2] = [
        LinkedStateDescriptor {
            rust_name: "Draft",
            label: None,
            description: None,
            has_data: false,
        },
        LinkedStateDescriptor {
            rust_name: "Review",
            label: None,
            description: None,
            has_data: false,
        },
    ];
    static TRANSITIONS: [LinkedTransitionDescriptor; 2] = [
        LinkedTransitionDescriptor {
            method_name: "submit",
            from: "Draft",
            to: &["Review"],
            label: None,
            description: None,
        },
        LinkedTransitionDescriptor {
            method_name: "ghost",
            from: "Missing",
            to: &["Review"],
            label: None,
            description: None,
        },
    ];
    static LINKED: [LinkedMachineGraph; 1] = [LinkedMachineGraph {
        machine: MachineDescriptor {
            module_path: "broken",
            rust_type_path: "broken::Machine",
        },
        label: None,
        description: None,
        states: &STATES,
        transitions: LinkedTransitionInventory::new(transitions),
        static_links: &[],
    }];

    assert_eq!(
        CodebaseDoc::try_from_linked(&LINKED)
            .unwrap_err()
            .to_string(),
        "linked machine `broken::Machine` contains transition `ghost` whose source state is missing from the state list"
    );
}

#[test]
fn malformed_inventory_rejects_missing_static_link_source_state() {
    fn transitions() -> &'static [LinkedTransitionDescriptor] {
        &[]
    }

    static STATES: [LinkedStateDescriptor; 1] = [LinkedStateDescriptor {
        rust_name: "Draft",
        label: None,
        description: None,
        has_data: false,
    }];
    static LINKS: [StaticMachineLinkDescriptor; 1] = [StaticMachineLinkDescriptor {
        from_state: "Missing",
        field_name: None,
        to_machine_path: &["task", "Machine"],
        to_state: "Running",
    }];
    static LINKED: [LinkedMachineGraph; 1] = [LinkedMachineGraph {
        machine: MachineDescriptor {
            module_path: "broken",
            rust_type_path: "broken::Machine",
        },
        label: None,
        description: None,
        states: &STATES,
        transitions: LinkedTransitionInventory::new(transitions),
        static_links: &LINKS,
    }];

    assert_eq!(
        CodebaseDoc::try_from_linked(&LINKED)
            .unwrap_err()
            .to_string(),
        "linked machine `broken::Machine` contains a static payload link from missing source state `Missing`"
    );
}

#[test]
fn malformed_inventory_rejects_missing_validator_machine() {
    static VALIDATORS: [LinkedValidatorEntryDescriptor; 1] = [LinkedValidatorEntryDescriptor {
        machine: MachineDescriptor {
            module_path: "broken",
            rust_type_path: "broken::Machine",
        },
        source_module_path: "broken",
        source_type_display: "BrokenRow",
        target_states: &["Draft"],
    }];

    assert_eq!(
        CodebaseDoc::try_from_linked_with_validator_entries(&[], &VALIDATORS)
            .unwrap_err()
            .to_string(),
        "linked validator entry `BrokenRow::into_machine()` from module `broken` points at missing machine `broken::Machine`"
    );
}

#[test]
fn malformed_inventory_rejects_missing_validator_target_state() {
    fn transitions() -> &'static [LinkedTransitionDescriptor] {
        &[]
    }

    static STATES: [LinkedStateDescriptor; 1] = [LinkedStateDescriptor {
        rust_name: "Draft",
        label: None,
        description: None,
        has_data: false,
    }];
    static LINKED: [LinkedMachineGraph; 1] = [LinkedMachineGraph {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
        },
        label: None,
        description: None,
        states: &STATES,
        transitions: LinkedTransitionInventory::new(transitions),
        static_links: &[],
    }];
    static VALIDATORS: [LinkedValidatorEntryDescriptor; 1] = [LinkedValidatorEntryDescriptor {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
        },
        source_module_path: "workflow",
        source_type_display: "DbRow",
        target_states: &["Missing"],
    }];

    assert_eq!(
        CodebaseDoc::try_from_linked_with_validator_entries(&LINKED, &VALIDATORS)
            .unwrap_err()
            .to_string(),
        "linked validator entry `DbRow::into_machine()` from module `workflow` points at missing state `workflow::Machine::Missing`"
    );
}

#[test]
fn malformed_inventory_rejects_empty_validator_target_set() {
    fn transitions() -> &'static [LinkedTransitionDescriptor] {
        &[]
    }

    static STATES: [LinkedStateDescriptor; 1] = [LinkedStateDescriptor {
        rust_name: "Draft",
        label: None,
        description: None,
        has_data: false,
    }];
    static LINKED: [LinkedMachineGraph; 1] = [LinkedMachineGraph {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
        },
        label: None,
        description: None,
        states: &STATES,
        transitions: LinkedTransitionInventory::new(transitions),
        static_links: &[],
    }];
    static VALIDATORS: [LinkedValidatorEntryDescriptor; 1] = [LinkedValidatorEntryDescriptor {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
        },
        source_module_path: "workflow",
        source_type_display: "DbRow",
        target_states: &[],
    }];

    assert_eq!(
        CodebaseDoc::try_from_linked_with_validator_entries(&LINKED, &VALIDATORS)
            .unwrap_err()
            .to_string(),
        "linked validator entry `DbRow::into_machine()` from module `workflow` for machine `workflow::Machine` contains no target states"
    );
}

#[test]
fn malformed_inventory_rejects_duplicate_validator_target_state() {
    fn transitions() -> &'static [LinkedTransitionDescriptor] {
        &[]
    }

    static STATES: [LinkedStateDescriptor; 1] = [LinkedStateDescriptor {
        rust_name: "Draft",
        label: None,
        description: None,
        has_data: false,
    }];
    static LINKED: [LinkedMachineGraph; 1] = [LinkedMachineGraph {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
        },
        label: None,
        description: None,
        states: &STATES,
        transitions: LinkedTransitionInventory::new(transitions),
        static_links: &[],
    }];
    static VALIDATORS: [LinkedValidatorEntryDescriptor; 1] = [LinkedValidatorEntryDescriptor {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
        },
        source_module_path: "workflow",
        source_type_display: "DbRow",
        target_states: &["Draft", "Draft"],
    }];

    assert_eq!(
        CodebaseDoc::try_from_linked_with_validator_entries(&LINKED, &VALIDATORS)
            .unwrap_err()
            .to_string(),
        "linked validator entry `DbRow::into_machine()` from module `workflow` for machine `workflow::Machine` contains duplicate target state `Draft`"
    );
}

#[test]
fn malformed_inventory_rejects_duplicate_validator_entry_identity() {
    fn transitions() -> &'static [LinkedTransitionDescriptor] {
        &[]
    }

    static STATES: [LinkedStateDescriptor; 1] = [LinkedStateDescriptor {
        rust_name: "Draft",
        label: None,
        description: None,
        has_data: false,
    }];
    static LINKED: [LinkedMachineGraph; 1] = [LinkedMachineGraph {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
        },
        label: None,
        description: None,
        states: &STATES,
        transitions: LinkedTransitionInventory::new(transitions),
        static_links: &[],
    }];
    static VALIDATORS: [LinkedValidatorEntryDescriptor; 2] = [
        LinkedValidatorEntryDescriptor {
            machine: MachineDescriptor {
                module_path: "workflow",
                rust_type_path: "workflow::Machine",
            },
            source_module_path: "workflow",
            source_type_display: "DbRow",
            target_states: &["Draft"],
        },
        LinkedValidatorEntryDescriptor {
            machine: MachineDescriptor {
                module_path: "workflow",
                rust_type_path: "workflow::Machine",
            },
            source_module_path: "workflow",
            source_type_display: "DbRow",
            target_states: &["Draft"],
        },
    ];

    assert_eq!(
        CodebaseDoc::try_from_linked_with_validator_entries(&LINKED, &VALIDATORS)
            .unwrap_err()
            .to_string(),
        "linked validator entry `DbRow::into_machine()` from module `workflow` appears more than once for machine `workflow::Machine`"
    );
}
