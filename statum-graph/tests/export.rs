#![allow(dead_code)]

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use statum::{
    MachineDescriptor, MachineGraph, MachinePresentation, MachinePresentationDescriptor,
    StateDescriptor, StatePresentation, TransitionDescriptor, TransitionInventory,
    TransitionPresentation, TransitionPresentationInventory,
};
use statum_graph::{render, ExportDocError, MachineDoc, MachineDocError};

mod linear {
    use statum::{machine, state, transition};

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ReviewPayload {
        pub reviewer: &'static str,
    }

    #[state]
    pub enum State {
        Draft,
        Review(ReviewPayload),
        Published,
    }

    #[machine]
    pub struct Flow<State> {}

    #[transition]
    impl Flow<Draft> {
        fn submit(self) -> Flow<Review> {
            self.transition_with(ReviewPayload { reviewer: "amy" })
        }
    }

    #[transition]
    impl Flow<Review> {
        fn publish(self) -> Flow<Published> {
            self.transition()
        }
    }
}

mod branching {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Draft,
        Review,
        Accepted,
        Rejected,
        Archived,
    }

    #[machine]
    pub struct Flow<State> {}

    #[transition]
    impl Flow<Draft> {
        fn submit(self) -> Flow<Review> {
            self.transition()
        }
    }

    #[transition]
    impl Flow<Review> {
        fn maybe_decide(
            self,
            accept: bool,
        ) -> ::core::result::Result<Flow<Accepted>, Flow<Rejected>> {
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

    #[transition]
    impl Flow<Accepted> {
        fn archive(self) -> Flow<Archived> {
            self.transition()
        }
    }

    #[transition]
    impl Flow<Rejected> {
        fn archive(self) -> Flow<Archived> {
            self.transition()
        }
    }
}

mod multi_root {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        First,
        Second,
        Finished,
    }

    #[machine]
    pub struct Flow<State> {}

    #[transition]
    impl Flow<First> {
        fn finish(self) -> Flow<Finished> {
            self.transition()
        }
    }
}

mod no_root {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Draft,
        Review,
        Rejected,
    }

    #[machine]
    pub struct Flow<State> {}

    #[transition]
    impl Flow<Draft> {
        fn submit(self) -> Flow<Review> {
            self.transition()
        }
    }

    #[transition]
    impl Flow<Review> {
        fn reject(self) -> Flow<Rejected> {
            self.transition()
        }
    }

    #[transition]
    impl Flow<Rejected> {
        fn rework(self) -> Flow<Draft> {
            self.transition()
        }
    }
}

mod macro_generated {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Start,
        Enabled,
        MacroTarget,
    }

    #[machine]
    pub struct Flow<State> {}

    #[transition]
    impl Flow<Start> {
        fn enable(self) -> Flow<Enabled> {
            self.transition()
        }
    }

    macro_rules! generated_transitions {
        () => {
            #[transition]
            impl Flow<Enabled> {
                fn via_macro(self) -> Flow<MacroTarget> {
                    self.transition()
                }
            }
        };
    }

    generated_transitions!();
}

mod presented {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        #[present(label = "Queued", description = "Waiting for work.")]
        Queued,
        #[present(label = "Running")]
        Running,
        Done,
    }

    #[machine]
    #[present(
        label = "Presented Flow",
        description = "Presentation metadata for renderer output."
    )]
    pub struct Flow<State> {}

    #[transition]
    impl Flow<Queued> {
        #[present(label = "Start", description = "Begin running queued work.")]
        fn start(self) -> Flow<Running> {
            self.transition()
        }
    }

    #[transition]
    impl Flow<Running> {
        #[present(label = "Finish")]
        fn finish(self) -> Flow<Done> {
            self.transition()
        }
    }
}

#[test]
fn exports_linear_machine_topology_from_graph() {
    let doc = MachineDoc::from_machine::<linear::Flow<linear::Draft>>();

    assert_eq!(doc.machine().rust_type_path, "export::linear::Flow");
    assert_eq!(
        doc.states()
            .iter()
            .map(|state| (
                state.descriptor.rust_name,
                state.descriptor.has_data,
                state.is_root
            ))
            .collect::<Vec<_>>(),
        vec![
            ("Draft", false, true),
            ("Review", true, false),
            ("Published", false, false),
        ]
    );
    assert_eq!(
        doc.edges()
            .iter()
            .map(|edge| edge.descriptor.method_name)
            .collect::<Vec<_>>(),
        vec!["submit", "publish"]
    );
}

#[test]
fn preserves_exact_branch_targets_and_sorts_edges_stably() {
    let doc = MachineDoc::from_machine::<branching::Flow<branching::Review>>();

    assert_eq!(
        doc.edges()
            .iter()
            .map(|edge| edge.descriptor.method_name)
            .collect::<Vec<_>>(),
        vec![
            "submit",
            "accept",
            "maybe_decide",
            "reject",
            "archive",
            "archive"
        ]
    );

    let maybe_decide = doc
        .edges()
        .iter()
        .find(|edge| edge.descriptor.method_name == "maybe_decide")
        .expect("branching transition");
    assert_eq!(
        maybe_decide
            .descriptor
            .to
            .iter()
            .map(|state| doc.state(*state).unwrap().descriptor.rust_name)
            .collect::<Vec<_>>(),
        vec!["Accepted", "Rejected"]
    );
}

#[test]
fn derives_multiple_roots_and_zero_roots_from_topology() {
    let multi_root = MachineDoc::from_machine::<multi_root::Flow<multi_root::First>>();
    assert_eq!(
        multi_root
            .roots()
            .map(|state| state.descriptor.rust_name)
            .collect::<Vec<_>>(),
        vec!["First", "Second"]
    );

    let no_root = MachineDoc::from_machine::<no_root::Flow<no_root::Draft>>();
    assert_eq!(no_root.roots().count(), 0);
}

#[test]
fn mermaid_snapshot_is_stable_for_reconverging_graphs() {
    let doc = MachineDoc::from_machine::<branching::Flow<branching::Draft>>();
    insta::assert_snapshot!("branching_flow_mermaid", render::mermaid(&doc));
}

#[test]
fn mermaid_renders_one_edge_per_legal_target() {
    let doc = MachineDoc::from_machine::<branching::Flow<branching::Draft>>();
    let mermaid = render::mermaid(&doc);

    assert_eq!(mermaid.matches("-->|maybe_decide|").count(), 2);
    assert!(mermaid.contains("s1 -->|maybe_decide| s2"));
    assert!(mermaid.contains("s1 -->|maybe_decide| s3"));
}

#[test]
fn export_doc_joins_generated_presentation_labels_and_descriptions() {
    let doc = MachineDoc::from_machine::<presented::Flow<presented::Queued>>();
    let export = doc
        .export_with_presentation(&presented::flow::PRESENTATION)
        .expect("generated presentation should join cleanly");

    assert_eq!(
        export.machine(),
        statum_graph::ExportMachine {
            module_path: "export::presented",
            rust_type_path: "export::presented::Flow",
            label: Some("Presented Flow"),
            description: Some("Presentation metadata for renderer output."),
        }
    );
    assert_eq!(
        export
            .states()
            .iter()
            .map(|state| (state.rust_name, state.label, state.description))
            .collect::<Vec<_>>(),
        vec![
            ("Queued", Some("Queued"), Some("Waiting for work.")),
            ("Running", Some("Running"), None),
            ("Done", None, None),
        ]
    );
    assert_eq!(
        export
            .transitions()
            .iter()
            .map(|transition| {
                (
                    transition.method_name,
                    transition.label,
                    transition.description,
                    transition.from,
                    transition.to.clone(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (
                "start",
                Some("Start"),
                Some("Begin running queued work."),
                0,
                vec![1]
            ),
            ("finish", Some("Finish"), None, 1, vec![2]),
        ]
    );
}

#[test]
fn dot_snapshot_is_stable_for_reconverging_graphs() {
    let doc = MachineDoc::from_machine::<branching::Flow<branching::Draft>>();
    insta::assert_snapshot!("branching_flow_dot", render::dot(&doc));
}

#[test]
fn plantuml_snapshot_is_stable_for_reconverging_graphs() {
    let doc = MachineDoc::from_machine::<branching::Flow<branching::Draft>>();
    insta::assert_snapshot!("branching_flow_plantuml", render::plantuml(&doc));
}

#[test]
fn json_snapshot_is_stable_for_presentation_overlay() {
    let doc = MachineDoc::from_machine::<presented::Flow<presented::Queued>>();
    let export = doc
        .export_with_presentation(&presented::flow::PRESENTATION)
        .expect("generated presentation should join cleanly");

    insta::assert_snapshot!("presented_flow_json", render::json(&export));
}

#[test]
fn format_write_to_creates_parent_dirs_and_writes_requested_format() {
    let doc = MachineDoc::from_machine::<presented::Flow<presented::Queued>>();
    let export = doc
        .export_with_presentation(&presented::flow::PRESENTATION)
        .expect("generated presentation should join cleanly");
    let tempdir = tempfile::tempdir().expect("tempdir");
    let path = tempdir.path().join("nested/graph.json");

    let written = render::Format::Json
        .write_to(&export, &path)
        .expect("json output should write");

    assert_eq!(written, path);
    assert_eq!(
        fs::read_to_string(&path).expect("json file should exist"),
        render::json(&export)
    );
}

#[test]
fn write_all_to_dir_writes_every_format_with_stable_extensions() {
    let doc = MachineDoc::from_machine::<branching::Flow<branching::Draft>>();
    let tempdir = tempfile::tempdir().expect("tempdir");
    let bundle_dir = tempdir.path().join("bundle");

    let paths =
        render::write_all_to_dir(&doc, &bundle_dir, "flow").expect("bundle output should write");

    assert_eq!(
        paths,
        vec![
            bundle_dir.join("flow.mmd"),
            bundle_dir.join("flow.dot"),
            bundle_dir.join("flow.puml"),
            bundle_dir.join("flow.json"),
        ]
    );
    assert_eq!(
        fs::read_to_string(&paths[0]).expect("mermaid file should exist"),
        render::mermaid(&doc)
    );
    assert_eq!(
        fs::read_to_string(&paths[1]).expect("dot file should exist"),
        render::dot(&doc)
    );
    assert_eq!(
        fs::read_to_string(&paths[2]).expect("plantuml file should exist"),
        render::plantuml(&doc)
    );
    assert_eq!(
        fs::read_to_string(&paths[3]).expect("json file should exist"),
        render::json(&doc)
    );
}

#[test]
fn write_all_to_dir_rejects_path_like_stem() {
    let doc = MachineDoc::from_machine::<branching::Flow<branching::Draft>>();
    let tempdir = tempfile::tempdir().expect("tempdir");
    let bundle_dir = tempdir.path().join("bundle");
    let outside = tempdir.path().join("escape.mmd");
    let stem = Path::new("..").join("escape");

    let error = render::write_all_to_dir(&doc, &bundle_dir, stem.to_str().expect("utf-8 stem"))
        .expect_err("path-like stem should be rejected");

    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert!(!bundle_dir.exists());
    assert!(!outside.exists());
}

#[test]
fn exports_macro_generated_transition_sites() {
    let doc = MachineDoc::from_machine::<macro_generated::Flow<macro_generated::Enabled>>();

    assert_eq!(
        doc.edges()
            .iter()
            .map(|edge| edge.descriptor.method_name)
            .collect::<Vec<_>>(),
        vec!["enable", "via_macro"]
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum InvalidStateId {
    Draft,
    Published,
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum InvalidTransitionId {
    Submit,
    Publish,
    Archive,
}

static VALID_STATE_DESCRIPTORS: [StateDescriptor<InvalidStateId>; 2] = [
    StateDescriptor {
        id: InvalidStateId::Draft,
        rust_name: "Draft",
        has_data: false,
    },
    StateDescriptor {
        id: InvalidStateId::Published,
        rust_name: "Published",
        has_data: false,
    },
];

static EMPTY_STATE_DESCRIPTORS: [StateDescriptor<InvalidStateId>; 0] = [];
static EMPTY_TARGET_IDS: [InvalidStateId; 0] = [];

static DUPLICATE_STATE_DESCRIPTORS: [StateDescriptor<InvalidStateId>; 2] = [
    StateDescriptor {
        id: InvalidStateId::Draft,
        rust_name: "Draft",
        has_data: false,
    },
    StateDescriptor {
        id: InvalidStateId::Draft,
        rust_name: "DraftDuplicate",
        has_data: false,
    },
];

static INVALID_TARGETS: [InvalidStateId; 1] = [InvalidStateId::Missing];
static VALID_PUBLISHED_TARGET: [InvalidStateId; 1] = [InvalidStateId::Published];
static DUPLICATE_PUBLISHED_TARGETS: [InvalidStateId; 2] =
    [InvalidStateId::Published, InvalidStateId::Published];

static INVALID_SOURCE_TRANSITIONS: [TransitionDescriptor<InvalidStateId, InvalidTransitionId>; 1] =
    [TransitionDescriptor {
        id: InvalidTransitionId::Submit,
        method_name: "submit",
        from: InvalidStateId::Missing,
        to: &INVALID_TARGETS,
    }];

static INVALID_TARGET_TRANSITIONS: [TransitionDescriptor<InvalidStateId, InvalidTransitionId>; 1] =
    [TransitionDescriptor {
        id: InvalidTransitionId::Submit,
        method_name: "submit",
        from: InvalidStateId::Draft,
        to: &INVALID_TARGETS,
    }];

static PIPE_LABEL_TRANSITIONS: [TransitionDescriptor<InvalidStateId, InvalidTransitionId>; 1] =
    [TransitionDescriptor {
        id: InvalidTransitionId::Submit,
        method_name: "submit|review",
        from: InvalidStateId::Draft,
        to: &VALID_PUBLISHED_TARGET,
    }];

static DUPLICATE_TRANSITION_ID_TRANSITIONS: [TransitionDescriptor<
    InvalidStateId,
    InvalidTransitionId,
>; 2] = [
    TransitionDescriptor {
        id: InvalidTransitionId::Submit,
        method_name: "submit",
        from: InvalidStateId::Draft,
        to: &VALID_PUBLISHED_TARGET,
    },
    TransitionDescriptor {
        id: InvalidTransitionId::Submit,
        method_name: "publish",
        from: InvalidStateId::Published,
        to: &VALID_PUBLISHED_TARGET,
    },
];

static DUPLICATE_TARGET_TRANSITIONS: [TransitionDescriptor<InvalidStateId, InvalidTransitionId>;
    1] = [TransitionDescriptor {
    id: InvalidTransitionId::Publish,
    method_name: "branch",
    from: InvalidStateId::Draft,
    to: &DUPLICATE_PUBLISHED_TARGETS,
}];

static DUPLICATE_TRANSITION_SITE_TRANSITIONS: [TransitionDescriptor<
    InvalidStateId,
    InvalidTransitionId,
>; 2] = [
    TransitionDescriptor {
        id: InvalidTransitionId::Submit,
        method_name: "review",
        from: InvalidStateId::Draft,
        to: &VALID_PUBLISHED_TARGET,
    },
    TransitionDescriptor {
        id: InvalidTransitionId::Archive,
        method_name: "review",
        from: InvalidStateId::Draft,
        to: &VALID_PUBLISHED_TARGET,
    },
];

fn invalid_source_transitions(
) -> &'static [TransitionDescriptor<InvalidStateId, InvalidTransitionId>] {
    &INVALID_SOURCE_TRANSITIONS
}

fn invalid_target_transitions(
) -> &'static [TransitionDescriptor<InvalidStateId, InvalidTransitionId>] {
    &INVALID_TARGET_TRANSITIONS
}

fn pipe_label_transitions() -> &'static [TransitionDescriptor<InvalidStateId, InvalidTransitionId>]
{
    &PIPE_LABEL_TRANSITIONS
}

fn duplicate_transition_id_transitions(
) -> &'static [TransitionDescriptor<InvalidStateId, InvalidTransitionId>] {
    &DUPLICATE_TRANSITION_ID_TRANSITIONS
}

fn duplicate_target_transitions(
) -> &'static [TransitionDescriptor<InvalidStateId, InvalidTransitionId>] {
    &DUPLICATE_TARGET_TRANSITIONS
}

fn duplicate_transition_site_transitions(
) -> &'static [TransitionDescriptor<InvalidStateId, InvalidTransitionId>] {
    &DUPLICATE_TRANSITION_SITE_TRANSITIONS
}

static INVALID_SOURCE_GRAPH: MachineGraph<InvalidStateId, InvalidTransitionId> = MachineGraph {
    machine: MachineDescriptor {
        module_path: "tests::invalid_source",
        rust_type_path: "tests::invalid_source::Flow",
    },
    states: &VALID_STATE_DESCRIPTORS,
    transitions: TransitionInventory::new(invalid_source_transitions),
};

static INVALID_TARGET_GRAPH: MachineGraph<InvalidStateId, InvalidTransitionId> = MachineGraph {
    machine: MachineDescriptor {
        module_path: "tests::invalid_target",
        rust_type_path: "tests::invalid_target::Flow",
    },
    states: &VALID_STATE_DESCRIPTORS,
    transitions: TransitionInventory::new(invalid_target_transitions),
};

static DUPLICATE_STATE_GRAPH: MachineGraph<InvalidStateId, InvalidTransitionId> = MachineGraph {
    machine: MachineDescriptor {
        module_path: "tests::duplicate_state",
        rust_type_path: "tests::duplicate_state::Flow",
    },
    states: &DUPLICATE_STATE_DESCRIPTORS,
    transitions: TransitionInventory::new(invalid_target_transitions),
};

static PIPE_LABEL_GRAPH: MachineGraph<InvalidStateId, InvalidTransitionId> = MachineGraph {
    machine: MachineDescriptor {
        module_path: "tests::pipe_label",
        rust_type_path: "tests::pipe_label::Flow",
    },
    states: &VALID_STATE_DESCRIPTORS,
    transitions: TransitionInventory::new(pipe_label_transitions),
};

static DUPLICATE_TRANSITION_ID_GRAPH: MachineGraph<InvalidStateId, InvalidTransitionId> =
    MachineGraph {
        machine: MachineDescriptor {
            module_path: "tests::duplicate_transition_id",
            rust_type_path: "tests::duplicate_transition_id::Flow",
        },
        states: &VALID_STATE_DESCRIPTORS,
        transitions: TransitionInventory::new(duplicate_transition_id_transitions),
    };

static DUPLICATE_TARGET_GRAPH: MachineGraph<InvalidStateId, InvalidTransitionId> = MachineGraph {
    machine: MachineDescriptor {
        module_path: "tests::duplicate_target",
        rust_type_path: "tests::duplicate_target::Flow",
    },
    states: &VALID_STATE_DESCRIPTORS,
    transitions: TransitionInventory::new(duplicate_target_transitions),
};

static DUPLICATE_TRANSITION_SITE_GRAPH: MachineGraph<InvalidStateId, InvalidTransitionId> =
    MachineGraph {
        machine: MachineDescriptor {
            module_path: "tests::duplicate_transition_site",
            rust_type_path: "tests::duplicate_transition_site::Flow",
        },
        states: &VALID_STATE_DESCRIPTORS,
        transitions: TransitionInventory::new(duplicate_transition_site_transitions),
    };

static FLAKY_INVENTORY_LOCK: Mutex<()> = Mutex::new(());
static FLAKY_TRANSITION_CALLS: AtomicUsize = AtomicUsize::new(0);

static FLAKY_VALID_TRANSITIONS: [TransitionDescriptor<InvalidStateId, InvalidTransitionId>; 1] =
    [TransitionDescriptor {
        id: InvalidTransitionId::Submit,
        method_name: "submit",
        from: InvalidStateId::Draft,
        to: &VALID_PUBLISHED_TARGET,
    }];

static EMPTY_TRANSITIONS: [TransitionDescriptor<InvalidStateId, InvalidTransitionId>; 0] = [];
static EMPTY_TARGET_TRANSITIONS: [TransitionDescriptor<InvalidStateId, InvalidTransitionId>; 1] =
    [TransitionDescriptor {
        id: InvalidTransitionId::Submit,
        method_name: "submit",
        from: InvalidStateId::Draft,
        to: &EMPTY_TARGET_IDS,
    }];

fn flaky_transitions() -> &'static [TransitionDescriptor<InvalidStateId, InvalidTransitionId>] {
    let call = FLAKY_TRANSITION_CALLS.fetch_add(1, Ordering::SeqCst);
    if call.is_multiple_of(2) {
        &FLAKY_VALID_TRANSITIONS
    } else {
        &EMPTY_TRANSITIONS
    }
}

fn empty_target_transitions() -> &'static [TransitionDescriptor<InvalidStateId, InvalidTransitionId>]
{
    &EMPTY_TARGET_TRANSITIONS
}

fn valid_transitions() -> &'static [TransitionDescriptor<InvalidStateId, InvalidTransitionId>] {
    &FLAKY_VALID_TRANSITIONS
}

static FLAKY_GRAPH: MachineGraph<InvalidStateId, InvalidTransitionId> = MachineGraph {
    machine: MachineDescriptor {
        module_path: "tests::flaky_inventory",
        rust_type_path: "tests::flaky_inventory::Flow",
    },
    states: &VALID_STATE_DESCRIPTORS,
    transitions: TransitionInventory::new(flaky_transitions),
};

static EMPTY_STATE_GRAPH: MachineGraph<InvalidStateId, InvalidTransitionId> = MachineGraph {
    machine: MachineDescriptor {
        module_path: "tests::empty_state_list",
        rust_type_path: "tests::empty_state_list::Flow",
    },
    states: &EMPTY_STATE_DESCRIPTORS,
    transitions: TransitionInventory::new(|| &EMPTY_TRANSITIONS),
};

static EMPTY_TARGET_GRAPH: MachineGraph<InvalidStateId, InvalidTransitionId> = MachineGraph {
    machine: MachineDescriptor {
        module_path: "tests::empty_target_set",
        rust_type_path: "tests::empty_target_set::Flow",
    },
    states: &VALID_STATE_DESCRIPTORS,
    transitions: TransitionInventory::new(empty_target_transitions),
};

static VALID_GRAPH: MachineGraph<InvalidStateId, InvalidTransitionId> = MachineGraph {
    machine: MachineDescriptor {
        module_path: "tests::valid_presentation",
        rust_type_path: "tests::valid_presentation::Flow",
    },
    states: &VALID_STATE_DESCRIPTORS,
    transitions: TransitionInventory::new(valid_transitions),
};

static EMPTY_STATE_PRESENTATIONS: [StatePresentation<InvalidStateId>; 0] = [];
static UNKNOWN_STATE_PRESENTATIONS: [StatePresentation<InvalidStateId>; 1] = [StatePresentation {
    id: InvalidStateId::Missing,
    label: Some("Missing"),
    description: None,
    metadata: (),
}];
static DUPLICATE_STATE_PRESENTATIONS: [StatePresentation<InvalidStateId>; 2] = [
    StatePresentation {
        id: InvalidStateId::Draft,
        label: Some("Draft"),
        description: None,
        metadata: (),
    },
    StatePresentation {
        id: InvalidStateId::Draft,
        label: Some("Draft Again"),
        description: None,
        metadata: (),
    },
];
static DUPLICATE_EMPTY_STATE_PRESENTATIONS: [StatePresentation<InvalidStateId>; 2] = [
    StatePresentation {
        id: InvalidStateId::Draft,
        label: None,
        description: None,
        metadata: (),
    },
    StatePresentation {
        id: InvalidStateId::Draft,
        label: None,
        description: None,
        metadata: (),
    },
];

static EMPTY_TRANSITION_PRESENTATIONS: [TransitionPresentation<InvalidTransitionId>; 0] = [];
static UNKNOWN_TRANSITION_PRESENTATIONS: [TransitionPresentation<InvalidTransitionId>; 1] =
    [TransitionPresentation {
        id: InvalidTransitionId::Archive,
        label: Some("Archive"),
        description: None,
        metadata: (),
    }];
static DUPLICATE_TRANSITION_PRESENTATIONS: [TransitionPresentation<InvalidTransitionId>; 2] = [
    TransitionPresentation {
        id: InvalidTransitionId::Submit,
        label: Some("Submit"),
        description: None,
        metadata: (),
    },
    TransitionPresentation {
        id: InvalidTransitionId::Submit,
        label: Some("Submit Again"),
        description: None,
        metadata: (),
    },
];
static DUPLICATE_EMPTY_TRANSITION_PRESENTATIONS: [TransitionPresentation<InvalidTransitionId>; 2] = [
    TransitionPresentation {
        id: InvalidTransitionId::Submit,
        label: None,
        description: None,
        metadata: (),
    },
    TransitionPresentation {
        id: InvalidTransitionId::Submit,
        label: None,
        description: None,
        metadata: (),
    },
];

static UNKNOWN_STATE_PRESENTATION: MachinePresentation<InvalidStateId, InvalidTransitionId> =
    MachinePresentation {
        machine: Some(MachinePresentationDescriptor {
            label: Some("Invalid"),
            description: None,
            metadata: (),
        }),
        states: &UNKNOWN_STATE_PRESENTATIONS,
        transitions: TransitionPresentationInventory::new(|| &EMPTY_TRANSITION_PRESENTATIONS),
    };

static DUPLICATE_STATE_PRESENTATION: MachinePresentation<InvalidStateId, InvalidTransitionId> =
    MachinePresentation {
        machine: None,
        states: &DUPLICATE_STATE_PRESENTATIONS,
        transitions: TransitionPresentationInventory::new(|| &EMPTY_TRANSITION_PRESENTATIONS),
    };

static DUPLICATE_EMPTY_STATE_PRESENTATION: MachinePresentation<
    InvalidStateId,
    InvalidTransitionId,
> = MachinePresentation {
    machine: None,
    states: &DUPLICATE_EMPTY_STATE_PRESENTATIONS,
    transitions: TransitionPresentationInventory::new(|| &EMPTY_TRANSITION_PRESENTATIONS),
};

static UNKNOWN_TRANSITION_PRESENTATION: MachinePresentation<InvalidStateId, InvalidTransitionId> =
    MachinePresentation {
        machine: None,
        states: &EMPTY_STATE_PRESENTATIONS,
        transitions: TransitionPresentationInventory::new(|| &UNKNOWN_TRANSITION_PRESENTATIONS),
    };

static DUPLICATE_TRANSITION_PRESENTATION: MachinePresentation<InvalidStateId, InvalidTransitionId> =
    MachinePresentation {
        machine: None,
        states: &EMPTY_STATE_PRESENTATIONS,
        transitions: TransitionPresentationInventory::new(|| &DUPLICATE_TRANSITION_PRESENTATIONS),
    };

static DUPLICATE_EMPTY_TRANSITION_PRESENTATION: MachinePresentation<
    InvalidStateId,
    InvalidTransitionId,
> = MachinePresentation {
    machine: None,
    states: &EMPTY_STATE_PRESENTATIONS,
    transitions: TransitionPresentationInventory::new(|| &DUPLICATE_EMPTY_TRANSITION_PRESENTATIONS),
};

#[test]
fn rejects_external_graph_with_missing_transition_source() {
    assert_eq!(
        MachineDoc::try_from_graph(&INVALID_SOURCE_GRAPH),
        Err(MachineDocError::MissingSourceState {
            machine: "tests::invalid_source::Flow",
            transition: "submit",
        })
    );
}

#[test]
fn rejects_external_graph_with_empty_state_list() {
    assert_eq!(
        MachineDoc::try_from_graph(&EMPTY_STATE_GRAPH),
        Err(MachineDocError::EmptyStateList {
            machine: "tests::empty_state_list::Flow",
        })
    );
}

#[test]
fn rejects_external_graph_with_missing_transition_target() {
    assert_eq!(
        MachineDoc::try_from_graph(&INVALID_TARGET_GRAPH),
        Err(MachineDocError::MissingTargetState {
            machine: "tests::invalid_target::Flow",
            transition: "submit",
        })
    );
}

#[test]
fn rejects_external_graph_with_duplicate_state_ids() {
    assert_eq!(
        MachineDoc::try_from_graph(&DUPLICATE_STATE_GRAPH),
        Err(MachineDocError::DuplicateStateId {
            machine: "tests::duplicate_state::Flow",
            state: "DraftDuplicate",
        })
    );
}

#[test]
fn rejects_external_graph_with_empty_target_set() {
    assert_eq!(
        MachineDoc::try_from_graph(&EMPTY_TARGET_GRAPH),
        Err(MachineDocError::EmptyTargetSet {
            machine: "tests::empty_target_set::Flow",
            transition: "submit",
        })
    );
}

#[test]
fn mermaid_escapes_external_edge_labels() {
    let doc = MachineDoc::try_from_graph(&PIPE_LABEL_GRAPH)
        .expect("external graph with valid topology should export");
    let mermaid = render::mermaid(&doc);

    assert!(mermaid.contains("-->|submit&#124;review|"));
}

#[test]
fn rejects_external_graph_with_duplicate_transition_ids() {
    assert_eq!(
        MachineDoc::try_from_graph(&DUPLICATE_TRANSITION_ID_GRAPH),
        Err(MachineDocError::DuplicateTransitionId {
            machine: "tests::duplicate_transition_id::Flow",
            transition: "publish",
        })
    );
}

#[test]
fn rejects_external_graph_with_duplicate_target_states() {
    assert_eq!(
        MachineDoc::try_from_graph(&DUPLICATE_TARGET_GRAPH),
        Err(MachineDocError::DuplicateTargetState {
            machine: "tests::duplicate_target::Flow",
            transition: "branch",
            state: "Published",
        })
    );
}

#[test]
fn rejects_external_graph_with_duplicate_transition_sites() {
    assert_eq!(
        MachineDoc::try_from_graph(&DUPLICATE_TRANSITION_SITE_GRAPH),
        Err(MachineDocError::DuplicateTransitionSite {
            machine: "tests::duplicate_transition_site::Flow",
            state: "Draft",
            transition: "review",
        })
    );
}

#[test]
fn snapshots_external_transition_inventory_once_per_export() {
    let _guard = FLAKY_INVENTORY_LOCK.lock().expect("flaky inventory lock");
    FLAKY_TRANSITION_CALLS.store(0, Ordering::SeqCst);

    let doc = MachineDoc::try_from_graph(&FLAKY_GRAPH)
        .expect("flaky inventory should still export from one consistent snapshot");

    assert_eq!(
        doc.roots()
            .map(|state| state.descriptor.rust_name)
            .collect::<Vec<_>>(),
        vec!["Draft"]
    );
    assert_eq!(
        doc.edges()
            .iter()
            .map(|edge| edge.descriptor.method_name)
            .collect::<Vec<_>>(),
        vec!["submit"]
    );
}

#[test]
fn rejects_presentation_with_unknown_state_id() {
    let doc = MachineDoc::try_from_graph(&VALID_GRAPH).expect("valid external graph should export");

    assert_eq!(
        doc.export_with_presentation(&UNKNOWN_STATE_PRESENTATION),
        Err(ExportDocError::UnknownStatePresentation {
            machine: "tests::valid_presentation::Flow",
            entry: 0,
        })
    );
}

#[test]
fn rejects_presentation_with_duplicate_state_id() {
    let doc = MachineDoc::try_from_graph(&VALID_GRAPH).expect("valid external graph should export");

    assert_eq!(
        doc.export_with_presentation(&DUPLICATE_STATE_PRESENTATION),
        Err(ExportDocError::DuplicateStatePresentation {
            machine: "tests::valid_presentation::Flow",
            entry: 1,
        })
    );
}

#[test]
fn rejects_presentation_with_duplicate_state_id_when_first_entry_is_empty() {
    let doc = MachineDoc::try_from_graph(&VALID_GRAPH).expect("valid external graph should export");

    assert_eq!(
        doc.export_with_presentation(&DUPLICATE_EMPTY_STATE_PRESENTATION),
        Err(ExportDocError::DuplicateStatePresentation {
            machine: "tests::valid_presentation::Flow",
            entry: 1,
        })
    );
}

#[test]
fn rejects_presentation_with_unknown_transition_id() {
    let doc = MachineDoc::try_from_graph(&VALID_GRAPH).expect("valid external graph should export");

    assert_eq!(
        doc.export_with_presentation(&UNKNOWN_TRANSITION_PRESENTATION),
        Err(ExportDocError::UnknownTransitionPresentation {
            machine: "tests::valid_presentation::Flow",
            entry: 0,
        })
    );
}

#[test]
fn rejects_presentation_with_duplicate_transition_id() {
    let doc = MachineDoc::try_from_graph(&VALID_GRAPH).expect("valid external graph should export");

    assert_eq!(
        doc.export_with_presentation(&DUPLICATE_TRANSITION_PRESENTATION),
        Err(ExportDocError::DuplicateTransitionPresentation {
            machine: "tests::valid_presentation::Flow",
            entry: 1,
        })
    );
}

#[test]
fn rejects_presentation_with_duplicate_transition_id_when_first_entry_is_empty() {
    let doc = MachineDoc::try_from_graph(&VALID_GRAPH).expect("valid external graph should export");

    assert_eq!(
        doc.export_with_presentation(&DUPLICATE_EMPTY_TRANSITION_PRESENTATION),
        Err(ExportDocError::DuplicateTransitionPresentation {
            machine: "tests::valid_presentation::Flow",
            entry: 1,
        })
    );
}
