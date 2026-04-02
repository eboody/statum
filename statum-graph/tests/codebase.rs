#![allow(dead_code)]

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::OnceLock;

use statum::{
    LinkedMachineGraph, LinkedStateDescriptor, LinkedTransitionDescriptor,
    LinkedTransitionInventory, LinkedValidatorEntryDescriptor, MachineDescriptor, MachineRole,
    StaticMachineLinkDescriptor,
};
use statum_graph::{
    codebase::{render, CodebaseMachineRelationGroupSemantic, CodebaseMachineRole},
    CodebaseDoc,
};

fn broken_row_type_name() -> &'static str {
    "broken::BrokenRow"
}

fn workflow_db_row_type_name() -> &'static str {
    "workflow::DbRow"
}

fn zero_step_composition_linked() -> &'static [LinkedMachineGraph] {
    static LINKED: OnceLock<Box<[LinkedMachineGraph]>> = OnceLock::new();
    LINKED
        .get_or_init(|| {
            let states = Box::new([LinkedStateDescriptor {
                rust_name: "Idle",
                label: Some("Idle"),
                description: None,
                docs: None,
                has_data: false,
                direct_construction_available: true,
            }]);
            Box::new([LinkedMachineGraph {
                machine: MachineDescriptor {
                    module_path: "zero_step::machine",
                    rust_type_path: "zero_step::machine::Flow",
                    role: MachineRole::Composition,
                },
                label: Some("Zero Step Flow"),
                description: None,
                docs: None,
                states: Box::leak(states),
                transitions: LinkedTransitionInventory::new(zero_step_transitions),
                static_links: &[],
            }])
        })
        .as_ref()
}

fn zero_step_transitions() -> &'static [LinkedTransitionDescriptor] {
    &[]
}

fn too_many_journeys_linked() -> &'static [LinkedMachineGraph] {
    static LINKED: OnceLock<Box<[LinkedMachineGraph]>> = OnceLock::new();
    LINKED
        .get_or_init(|| {
            let depth = 9usize;
            let node_count = (1usize << (depth + 1)) - 1;
            let mut state_names = Vec::with_capacity(node_count);
            for index in 0..node_count {
                state_names.push(Box::leak(format!("S{index}").into_boxed_str()) as &'static str);
            }
            let states = state_names
                .iter()
                .map(|name| LinkedStateDescriptor {
                    rust_name: name,
                    label: Some(name),
                    description: None,
                    docs: None,
                    has_data: false,
                    direct_construction_available: true,
                })
                .collect::<Vec<_>>();
            let transitions = (0..((1usize << depth) - 1))
                .map(|index| {
                    let left = state_names[index * 2 + 1];
                    let right = state_names[index * 2 + 2];
                    let to = Box::leak(Box::new([left, right])) as &'static [&'static str; 2];
                    LinkedTransitionDescriptor {
                        method_name: Box::leak(format!("step_{index}").into_boxed_str()),
                        label: None,
                        description: None,
                        docs: None,
                        from: state_names[index],
                        to,
                    }
                })
                .collect::<Vec<_>>();
            TOO_MANY_JOURNEY_STATES
                .set(states.into_boxed_slice())
                .expect("set too-many-journey states once");
            TOO_MANY_JOURNEY_TRANSITIONS
                .set(transitions.into_boxed_slice())
                .expect("set too-many-journey transitions once");
            Box::new([LinkedMachineGraph {
                machine: MachineDescriptor {
                    module_path: "too_many::machine",
                    rust_type_path: "too_many::machine::Flow",
                    role: MachineRole::Composition,
                },
                label: Some("Too Many Journeys"),
                description: None,
                docs: None,
                states: too_many_journey_states(),
                transitions: LinkedTransitionInventory::new(too_many_journey_transitions),
                static_links: &[],
            }])
        })
        .as_ref()
}

static TOO_MANY_JOURNEY_STATES: OnceLock<Box<[LinkedStateDescriptor]>> = OnceLock::new();
static TOO_MANY_JOURNEY_TRANSITIONS: OnceLock<Box<[LinkedTransitionDescriptor]>> = OnceLock::new();

fn same_endpoint_journeys_linked() -> &'static [LinkedMachineGraph] {
    static LINKED: OnceLock<Box<[LinkedMachineGraph]>> = OnceLock::new();
    LINKED
        .get_or_init(|| {
            let states = Box::new([
                LinkedStateDescriptor {
                    rust_name: "Start",
                    label: Some("Start"),
                    description: None,
                    docs: None,
                    has_data: false,
                    direct_construction_available: true,
                },
                LinkedStateDescriptor {
                    rust_name: "ReviewA",
                    label: Some("Review A"),
                    description: None,
                    docs: None,
                    has_data: false,
                    direct_construction_available: true,
                },
                LinkedStateDescriptor {
                    rust_name: "ReviewB",
                    label: Some("Review B"),
                    description: None,
                    docs: None,
                    has_data: false,
                    direct_construction_available: true,
                },
                LinkedStateDescriptor {
                    rust_name: "Done",
                    label: Some("Done"),
                    description: None,
                    docs: None,
                    has_data: false,
                    direct_construction_available: true,
                },
            ]);
            let choose_to =
                Box::leak(Box::new(["ReviewA", "ReviewB"])) as &'static [&'static str; 2];
            let finish_a = Box::leak(Box::new(["Done"])) as &'static [&'static str; 1];
            let finish_b = Box::leak(Box::new(["Done"])) as &'static [&'static str; 1];
            let transitions = Box::new([
                LinkedTransitionDescriptor {
                    method_name: "choose",
                    label: Some("Choose"),
                    description: None,
                    docs: None,
                    from: "Start",
                    to: choose_to,
                },
                LinkedTransitionDescriptor {
                    method_name: "finish_a",
                    label: Some("Finish A"),
                    description: None,
                    docs: None,
                    from: "ReviewA",
                    to: finish_a,
                },
                LinkedTransitionDescriptor {
                    method_name: "finish_b",
                    label: Some("Finish B"),
                    description: None,
                    docs: None,
                    from: "ReviewB",
                    to: finish_b,
                },
            ]);
            SAME_ENDPOINT_JOURNEY_STATES
                .set(states)
                .expect("set same-endpoint journey states once");
            SAME_ENDPOINT_JOURNEY_TRANSITIONS
                .set(transitions)
                .expect("set same-endpoint journey transitions once");
            Box::new([LinkedMachineGraph {
                machine: MachineDescriptor {
                    module_path: "same_endpoints::machine",
                    rust_type_path: "same_endpoints::machine::Flow",
                    role: MachineRole::Composition,
                },
                label: Some("Same Endpoint Flow"),
                description: None,
                docs: None,
                states: same_endpoint_journey_states(),
                transitions: LinkedTransitionInventory::new(same_endpoint_journey_transitions),
                static_links: &[],
            }])
        })
        .as_ref()
}

static SAME_ENDPOINT_JOURNEY_STATES: OnceLock<Box<[LinkedStateDescriptor]>> = OnceLock::new();
static SAME_ENDPOINT_JOURNEY_TRANSITIONS: OnceLock<Box<[LinkedTransitionDescriptor]>> =
    OnceLock::new();

fn too_many_journey_states() -> &'static [LinkedStateDescriptor] {
    TOO_MANY_JOURNEY_STATES
        .get()
        .expect("too-many-journey states initialized")
        .as_ref()
}

fn too_many_journey_transitions() -> &'static [LinkedTransitionDescriptor] {
    TOO_MANY_JOURNEY_TRANSITIONS
        .get()
        .expect("too-many-journey transitions initialized")
        .as_ref()
}

fn same_endpoint_journey_states() -> &'static [LinkedStateDescriptor] {
    SAME_ENDPOINT_JOURNEY_STATES
        .get()
        .expect("same-endpoint journey states initialized")
        .as_ref()
}

fn same_endpoint_journey_transitions() -> &'static [LinkedTransitionDescriptor] {
    SAME_ENDPOINT_JOURNEY_TRANSITIONS
        .get()
        .expect("same-endpoint journey transitions initialized")
        .as_ref()
}

mod task {
    use statum::{machine, state, transition, validators, Error};

    #[state]
    pub enum State {
        Idle,
        /// Task execution is in progress.
        #[present(label = "Running", description = "Task execution is active.")]
        Running,
        Done,
    }

    /// Handles the task lifecycle from idle to done.
    #[machine]
    #[present(
        label = "Task Machine",
        description = "Owns the exact task execution lifecycle."
    )]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Idle> {
        /// Starts task execution.
        #[present(
            label = "Start Task",
            description = "Moves the task from idle into running work."
        )]
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

    /// Rebuilds task machines from persisted task rows.
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
    use super::task;
    use statum::{machine, state, transition, validators, Error};

    #[state]
    pub enum State {
        Draft,
        /// Workflow execution is delegated to a running task.
        #[present(
            label = "In Progress",
            description = "Work is currently delegated to a running task."
        )]
        InProgress(super::task::Machine<super::task::Running>),
        Complete,
    }

    /// Coordinates workflow progress around task execution.
    #[machine(role = composition)]
    #[present(
        label = "Workflow Machine",
        description = "Tracks workflow progress across task execution."
    )]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Draft> {
        /// Starts the workflow with a running task.
        #[present(
            label = "Start Workflow",
            description = "Begins workflow execution with a running task."
        )]
        fn start(
            self,
            running_task: super::task::Machine<super::task::Running>,
        ) -> Machine<InProgress> {
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

    /// Rebuilds workflow machines from persisted workflow rows.
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
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Pending {
            child: super::task::Machine<super::task::Done>,
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
    assert_eq!(workflow.role, CodebaseMachineRole::Composition);
    assert_eq!(workflow.label, Some("Workflow Machine"));
    assert_eq!(
        workflow.description,
        Some("Tracks workflow progress across task execution.")
    );
    assert_eq!(
        workflow.docs,
        Some("Coordinates workflow progress around task execution.")
    );
    assert_eq!(
        workflow
            .states
            .iter()
            .find(|state| state.rust_name == "InProgress")
            .map(|state| state.label),
        Some(Some("In Progress"))
    );
    assert_eq!(
        workflow
            .states
            .iter()
            .find(|state| state.rust_name == "InProgress")
            .and_then(|state| state.description),
        Some("Work is currently delegated to a running task.")
    );
    assert_eq!(
        workflow
            .states
            .iter()
            .find(|state| state.rust_name == "InProgress")
            .and_then(|state| state.docs),
        Some("Workflow execution is delegated to a running task.")
    );
    let workflow_start = workflow
        .transitions
        .iter()
        .find(|transition| transition.method_name == "start")
        .expect("workflow start transition");
    assert_eq!(
        workflow_start.description,
        Some("Begins workflow execution with a running task.")
    );
    assert_eq!(
        workflow_start.docs,
        Some("Starts the workflow with a running task.")
    );
    assert_eq!(workflow.validator_entries.len(), 1);
    assert_eq!(
        workflow.validator_entries[0].source_type_display,
        "WorkflowRow"
    );
    assert_eq!(workflow.validator_entries[0].target_states, vec![0, 1, 2]);
    assert_eq!(
        workflow.validator_entries[0].docs,
        Some("Rebuilds workflow machines from persisted workflow rows.")
    );

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
        target_machine.description,
        Some("Owns the exact task execution lifecycle.")
    );
    assert_eq!(
        target_machine.docs,
        Some("Handles the task lifecycle from idle to done.")
    );
    assert_eq!(
        target_machine.validator_entries[0].display_label().as_ref(),
        "TaskRow::into_machine()"
    );
    assert_eq!(
        target_machine.validator_entries[0].docs,
        Some("Rebuilds task machines from persisted task rows.")
    );

    let relation_groups = doc.machine_relation_groups();
    let workflow_group = relation_groups
        .iter()
        .find(|group| {
            group.from_machine == workflow.index && group.to_machine == target_machine.index
        })
        .expect("workflow composition group");
    assert_eq!(
        workflow_group.semantic,
        CodebaseMachineRelationGroupSemantic::CompositionDirectChild
    );
    assert_eq!(
        workflow_group.display_label(),
        "composition refs: payload, param"
    );

    let named_holder = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("named_holder::Machine"))
        .expect("named holder machine");
    let named_group = relation_groups
        .iter()
        .find(|group| {
            group.from_machine == named_holder.index && group.to_machine == target_machine.index
        })
        .expect("named holder exact group");
    assert_eq!(
        named_group.semantic,
        CodebaseMachineRelationGroupSemantic::Exact
    );
    assert_eq!(named_group.display_label(), "exact refs: payload");
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
fn linked_codebase_machine_state_diagram_renders_selected_machine() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");
    let workflow = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("workflow::Machine"))
        .expect("workflow machine");
    let in_progress = workflow
        .states
        .iter()
        .find(|state| state.rust_name == "InProgress")
        .expect("workflow in-progress state");
    let in_progress_label = if in_progress.direct_construction_available {
        format!("{} [build]", in_progress.display_label())
    } else {
        in_progress.display_label().into_owned()
    };

    let mermaid = render::mermaid_machine_state(&doc, workflow.index)
        .expect("workflow machine state diagram");

    assert!(mermaid.contains("stateDiagram-v2"));
    assert!(mermaid.contains("%% Workflow Machine [composition]"));
    assert!(mermaid.contains(&format!(
        "state \"{}\" as {}",
        in_progress_label,
        workflow.node_id(in_progress.index)
    )));
    assert!(mermaid.contains(&format!("[*] --> {}", workflow.node_id(0))));
    assert!(mermaid.contains(&format!(
        "{} --> {} : Start Workflow",
        workflow.node_id(0),
        workflow.node_id(1)
    )));
    assert!(mermaid.contains(&format!("{} --> [*]", workflow.node_id(2))));
}

#[test]
fn linked_codebase_machine_journey_renders_selected_trace_as_state_diagram() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");
    let workflow = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("workflow::Machine"))
        .expect("workflow machine");

    let journeys = render::machine_journeys(&doc, workflow.index).expect("workflow journeys");
    assert_eq!(journeys.len(), 1);

    let mermaid = render::mermaid_machine_journey(&doc, workflow.index, &journeys[0].id)
        .expect("workflow journey diagram");

    assert!(mermaid.contains("stateDiagram-v2"));
    assert!(mermaid.contains("%% journey Workflow Machine [composition] ::"));
    assert!(mermaid.contains(&workflow.node_id(0)));
    assert!(mermaid.contains(&workflow.node_id(1)));
    assert!(mermaid.contains(&workflow.node_id(2)));
    assert!(mermaid.contains(": 1. Start Workflow"));
    assert!(mermaid.contains(": 2. finish"));
}

#[test]
fn zero_step_composition_machine_renders_one_state_journey() {
    let doc = CodebaseDoc::try_from_linked(zero_step_composition_linked()).expect("codebase doc");
    let machine = doc.machines().first().expect("zero-step machine");

    let journeys = render::machine_journeys(&doc, machine.index).expect("zero-step journeys");
    assert_eq!(journeys.len(), 1);
    assert!(journeys[0].steps().is_empty());

    let mermaid = render::mermaid_machine_journey(&doc, machine.index, &journeys[0].id)
        .expect("zero-step journey diagram");
    assert!(mermaid.contains("stateDiagram-v2"));
    assert!(mermaid.contains("[*] --> m0_s0"));
    assert!(mermaid.contains("m0_s0 --> [*]"));
}

#[test]
fn machine_journeys_use_step_sequence_identity_for_same_endpoint_variants() {
    let doc = CodebaseDoc::try_from_linked(same_endpoint_journeys_linked()).expect("codebase doc");
    let machine = doc
        .machines()
        .first()
        .expect("same-endpoint journeys machine");

    let journeys = render::machine_journeys(&doc, machine.index).expect("journeys");
    assert_eq!(journeys.len(), 2);
    assert!(journeys.iter().all(|journey| journey.ingress_state() == 0));
    assert!(journeys.iter().all(|journey| journey.egress_state == 3));
    assert_ne!(journeys[0].id, journeys[1].id);

    let first = render::mermaid_machine_journey(&doc, machine.index, &journeys[0].id)
        .expect("first journey diagram");
    let second = render::mermaid_machine_journey(&doc, machine.index, &journeys[1].id)
        .expect("second journey diagram");
    assert_ne!(first, second);
    assert!(
        (first.contains("Review A") && second.contains("Review B"))
            || (first.contains("Review B") && second.contains("Review A"))
    );
}

#[test]
fn machine_journeys_fail_closed_when_exact_enumeration_exceeds_budget() {
    let doc = CodebaseDoc::try_from_linked(too_many_journeys_linked()).expect("codebase doc");
    let machine = doc.machines().first().expect("too-many-journeys machine");

    let error = render::machine_journeys(&doc, machine.index)
        .expect_err("journey budget should fail closed");
    assert_eq!(
        error,
        render::DiagramError::TooManyJourneys {
            index: machine.index
        }
    );
}

#[test]
fn linked_codebase_workspace_flow_renders_machine_level_projection() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");

    let mermaid = render::mermaid_workspace_flow(
        &doc,
        render::WorkspaceFlowOptions {
            machine_indices: None,
            direction: render::WorkspaceFlowDirection::LeftRight,
            compact_labels: true,
            edge_labels: render::WorkspaceFlowEdgeLabelMode::Hidden,
            role_shapes: true,
        },
    )
    .expect("workspace flow diagram");

    assert!(mermaid.contains("graph LR"));
    assert!(mermaid.contains("[[\"Workflow\"]]"));
    assert!(mermaid.contains("Task"));
    assert!(!mermaid.contains("Draft"));
    assert!(!mermaid.contains("In Progress"));
    assert!(mermaid.contains("==>"));
    assert!(!mermaid.contains('|'));
}

#[test]
fn linked_codebase_workspace_flow_can_focus_on_selected_machine_subset() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");
    let workflow = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("workflow::Machine"))
        .expect("workflow machine");

    let mermaid = render::mermaid_workspace_flow(
        &doc,
        render::WorkspaceFlowOptions {
            machine_indices: Some(&[workflow.index]),
            ..render::WorkspaceFlowOptions::default()
        },
    )
    .expect("focused workspace flow diagram");

    assert!(mermaid.contains("graph TD"));
    assert!(mermaid.contains("Workflow Machine"));
    assert!(!mermaid.contains("Task Machine"));
    assert!(!mermaid.contains("==>"));
    assert!(!mermaid.contains("-->"));
}

#[test]
fn linked_codebase_workspace_flow_rejects_missing_selected_machine() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");

    assert_eq!(
        render::mermaid_workspace_flow(
            &doc,
            render::WorkspaceFlowOptions {
                machine_indices: Some(&[usize::MAX]),
                ..render::WorkspaceFlowOptions::default()
            },
        )
        .unwrap_err()
        .to_string(),
        format!("codebase machine index {} is missing", usize::MAX)
    );
}

#[test]
fn workspace_flow_disambiguates_duplicate_compact_machine_labels() {
    fn no_transitions() -> &'static [LinkedTransitionDescriptor] {
        &[]
    }

    static IDLE_STATE: [LinkedStateDescriptor; 1] = [LinkedStateDescriptor {
        rust_name: "Idle",
        label: Some("Idle"),
        description: None,
        docs: None,
        has_data: false,
        direct_construction_available: true,
    }];
    static LINKS: [StaticMachineLinkDescriptor; 1] = [StaticMachineLinkDescriptor {
        from_state: "Idle",
        field_name: Some("handoff"),
        to_machine_path: &["tasks", "broker", "machine", "Flow"],
        to_state: "Idle",
    }];
    static LINKED: [LinkedMachineGraph; 2] = [
        LinkedMachineGraph {
            machine: MachineDescriptor {
                module_path: "flows::broker::machine",
                rust_type_path: "flows::broker::machine::Flow",
                role: MachineRole::Protocol,
            },
            label: None,
            description: None,
            docs: None,
            states: &IDLE_STATE,
            transitions: LinkedTransitionInventory::new(no_transitions),
            static_links: &LINKS,
        },
        LinkedMachineGraph {
            machine: MachineDescriptor {
                module_path: "tasks::broker::machine",
                rust_type_path: "tasks::broker::machine::Flow",
                role: MachineRole::Protocol,
            },
            label: None,
            description: None,
            docs: None,
            states: &IDLE_STATE,
            transitions: LinkedTransitionInventory::new(no_transitions),
            static_links: &[],
        },
    ];

    let doc = CodebaseDoc::try_from_linked(&LINKED).expect("duplicate-label fixture");
    let mermaid = render::mermaid_workspace_flow(
        &doc,
        render::WorkspaceFlowOptions {
            compact_labels: true,
            edge_labels: render::WorkspaceFlowEdgeLabelMode::Hidden,
            ..render::WorkspaceFlowOptions::default()
        },
    )
    .expect("workspace flow diagram");

    assert!(mermaid.contains("flows::broker::machine::Flow"));
    assert!(mermaid.contains("tasks::broker::machine::Flow"));
    assert!(!mermaid.contains("m0[\"broker\"]\n    m1[\"broker\"]"));
}

#[test]
fn linked_codebase_relation_sequence_renders_direct_transition_param_handoff() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");
    let task = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("task::Machine"))
        .expect("task machine");
    let running = task
        .states
        .iter()
        .find(|state| state.rust_name == "Running")
        .expect("task running state");
    let running_label = if running.direct_construction_available {
        format!("{} [build]", running.display_label())
    } else {
        running.display_label().into_owned()
    };
    let workflow = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("workflow::Machine"))
        .expect("workflow machine");
    let start = workflow
        .transitions
        .iter()
        .find(|transition| transition.method_name == "start")
        .expect("workflow start transition");
    let relation = doc
        .outbound_relations_for_transition(workflow.index, start.index)
        .find(|relation| relation.attested_via.is_none())
        .expect("direct transition-param relation");

    let mermaid =
        render::mermaid_relation_sequence(&doc, relation.index).expect("direct relation sequence");

    assert!(mermaid.contains("sequenceDiagram"));
    assert!(mermaid.contains(&format!("participant m{} as Task Machine", task.index)));
    assert!(mermaid.contains(&format!(
        "participant m{} as Workflow Machine [composition]",
        workflow.index
    )));
    assert!(mermaid.contains(&format!(
        "m{}->>m{}: {} for Start Workflow",
        task.index, workflow.index, running_label
    )));
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
fn builder_markers_only_render_for_directly_constructible_states() {
    fn transitions() -> &'static [LinkedTransitionDescriptor] {
        &[]
    }

    static STATES: [LinkedStateDescriptor; 2] = [
        LinkedStateDescriptor {
            rust_name: "Draft",
            label: None,
            description: None,
            docs: None,
            has_data: false,
            direct_construction_available: false,
        },
        LinkedStateDescriptor {
            rust_name: "Review",
            label: None,
            description: None,
            docs: None,
            has_data: true,
            direct_construction_available: true,
        },
    ];
    static LINKED: [LinkedMachineGraph; 1] = [LinkedMachineGraph {
        machine: MachineDescriptor {
            module_path: "builder_markers",
            rust_type_path: "builder_markers::Machine",
            role: MachineRole::Protocol,
        },
        label: None,
        description: None,
        docs: None,
        states: &STATES,
        transitions: LinkedTransitionInventory::new(transitions),
        static_links: &[],
    }];

    let doc = CodebaseDoc::try_from_linked(&LINKED).expect("codebase doc");
    let mermaid = render::mermaid(&doc);
    let dot = render::dot(&doc);
    let plantuml = render::plantuml(&doc);

    assert!(mermaid.contains("Review (data) [build]"));
    assert!(!mermaid.contains("Draft [build]"));
    assert!(dot.contains("Review (data) [build]"));
    assert!(!dot.contains("Draft [build]"));
    assert!(plantuml.contains("Review (data) [build]"));
    assert!(!plantuml.contains("Draft [build]"));
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
            docs: None,
            has_data: false,
            direct_construction_available: true,
        },
        LinkedStateDescriptor {
            rust_name: "Review",
            label: None,
            description: None,
            docs: None,
            has_data: false,
            direct_construction_available: true,
        },
    ];
    static TRANSITIONS: [LinkedTransitionDescriptor; 2] = [
        LinkedTransitionDescriptor {
            method_name: "submit",
            from: "Draft",
            to: &["Review"],
            label: None,
            description: None,
            docs: None,
        },
        LinkedTransitionDescriptor {
            method_name: "ghost",
            from: "Missing",
            to: &["Review"],
            label: None,
            description: None,
            docs: None,
        },
    ];
    static LINKED: [LinkedMachineGraph; 1] = [LinkedMachineGraph {
        machine: MachineDescriptor {
            module_path: "broken",
            rust_type_path: "broken::Machine",
            role: MachineRole::Protocol,
        },
        label: None,
        description: None,
        docs: None,
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
        docs: None,
        has_data: false,
        direct_construction_available: true,
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
            role: MachineRole::Protocol,
        },
        label: None,
        description: None,
        docs: None,
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
            role: MachineRole::Protocol,
        },
        source_module_path: "broken",
        source_type_display: "BrokenRow",
        resolved_source_type_name: broken_row_type_name,
        docs: None,
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
        docs: None,
        has_data: false,
        direct_construction_available: true,
    }];
    static LINKED: [LinkedMachineGraph; 1] = [LinkedMachineGraph {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
            role: MachineRole::Protocol,
        },
        label: None,
        description: None,
        docs: None,
        states: &STATES,
        transitions: LinkedTransitionInventory::new(transitions),
        static_links: &[],
    }];
    static VALIDATORS: [LinkedValidatorEntryDescriptor; 1] = [LinkedValidatorEntryDescriptor {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
            role: MachineRole::Protocol,
        },
        source_module_path: "workflow",
        source_type_display: "DbRow",
        resolved_source_type_name: workflow_db_row_type_name,
        docs: None,
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
        docs: None,
        has_data: false,
        direct_construction_available: true,
    }];
    static LINKED: [LinkedMachineGraph; 1] = [LinkedMachineGraph {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
            role: MachineRole::Protocol,
        },
        label: None,
        description: None,
        docs: None,
        states: &STATES,
        transitions: LinkedTransitionInventory::new(transitions),
        static_links: &[],
    }];
    static VALIDATORS: [LinkedValidatorEntryDescriptor; 1] = [LinkedValidatorEntryDescriptor {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
            role: MachineRole::Protocol,
        },
        source_module_path: "workflow",
        source_type_display: "DbRow",
        resolved_source_type_name: workflow_db_row_type_name,
        docs: None,
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
        docs: None,
        has_data: false,
        direct_construction_available: true,
    }];
    static LINKED: [LinkedMachineGraph; 1] = [LinkedMachineGraph {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
            role: MachineRole::Protocol,
        },
        label: None,
        description: None,
        docs: None,
        states: &STATES,
        transitions: LinkedTransitionInventory::new(transitions),
        static_links: &[],
    }];
    static VALIDATORS: [LinkedValidatorEntryDescriptor; 1] = [LinkedValidatorEntryDescriptor {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
            role: MachineRole::Protocol,
        },
        source_module_path: "workflow",
        source_type_display: "DbRow",
        resolved_source_type_name: workflow_db_row_type_name,
        docs: None,
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
        docs: None,
        has_data: false,
        direct_construction_available: true,
    }];
    static LINKED: [LinkedMachineGraph; 1] = [LinkedMachineGraph {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
            role: MachineRole::Protocol,
        },
        label: None,
        description: None,
        docs: None,
        states: &STATES,
        transitions: LinkedTransitionInventory::new(transitions),
        static_links: &[],
    }];
    static VALIDATORS: [LinkedValidatorEntryDescriptor; 2] = [
        LinkedValidatorEntryDescriptor {
            machine: MachineDescriptor {
                module_path: "workflow",
                rust_type_path: "workflow::Machine",
                role: MachineRole::Protocol,
            },
            source_module_path: "workflow",
            source_type_display: "DbRow",
            resolved_source_type_name: workflow_db_row_type_name,
            docs: None,
            target_states: &["Draft"],
        },
        LinkedValidatorEntryDescriptor {
            machine: MachineDescriptor {
                module_path: "workflow",
                rust_type_path: "workflow::Machine",
                role: MachineRole::Protocol,
            },
            source_module_path: "workflow",
            source_type_display: "DbRow",
            resolved_source_type_name: workflow_db_row_type_name,
            docs: None,
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
