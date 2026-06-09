use statum_core::{
    GraphAuthorityLevel, GraphLintCode, MachineDescriptor, MachineGraph, StableGraphMetadata,
    StableGraphMetadataVersion, StateDescriptor, TransitionDescriptor, TransitionInventory,
    UnsupportedGraphMetadataCase,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum StateId {
    Draft,
    Review,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum TransitionId {
    Submit,
    Publish,
}

static REVIEW_TARGETS: [StateId; 1] = [StateId::Review];
static PUBLISH_TARGETS: [StateId; 1] = [StateId::Published];

static STATES: [StateDescriptor<StateId>; 3] = [
    StateDescriptor {
        id: StateId::Draft,
        rust_name: "Draft",
        has_data: false,
    },
    StateDescriptor {
        id: StateId::Review,
        rust_name: "Review",
        has_data: true,
    },
    StateDescriptor {
        id: StateId::Published,
        rust_name: "Published",
        has_data: false,
    },
];

static TRANSITIONS: [TransitionDescriptor<StateId, TransitionId>; 2] = [
    TransitionDescriptor {
        id: TransitionId::Submit,
        method_name: "submit",
        from: StateId::Draft,
        to: &REVIEW_TARGETS,
    },
    TransitionDescriptor {
        id: TransitionId::Publish,
        method_name: "publish",
        from: StateId::Review,
        to: &PUBLISH_TARGETS,
    },
];

static GRAPH: MachineGraph<StateId, TransitionId> = MachineGraph {
    machine: MachineDescriptor {
        module_path: "review_flow",
        rust_type_path: "review_flow::ReviewFlow",
    },
    states: &STATES,
    transitions: TransitionInventory::new(|| &TRANSITIONS),
};

#[test]
fn stable_metadata_from_graph_declares_version_authority_and_unsupported_cases() {
    let metadata = StableGraphMetadata::from_graph(&GRAPH);

    assert_eq!(metadata.version, StableGraphMetadataVersion::V1);
    assert_eq!(metadata.authority, GraphAuthorityLevel::CfgPrunedMacroInput);
    assert_eq!(
        metadata.unsupported_cases,
        vec![
            UnsupportedGraphMetadataCase::RuntimeOnlyTransitions,
            UnsupportedGraphMetadataCase::CfgAmbiguousAliases,
            UnsupportedGraphMetadataCase::UnexpandedCustomDecisionEnums,
            UnsupportedGraphMetadataCase::MacroGeneratedItems,
            UnsupportedGraphMetadataCase::IncludeGeneratedItems,
            UnsupportedGraphMetadataCase::FieldLevelPresentationMetadata,
        ]
    );

    assert_eq!(metadata.machine.rust_type_path, "review_flow::ReviewFlow");
    assert!(metadata.machine.fields.is_empty());
    assert_eq!(metadata.states[1].rust_name, "Review");
    assert!(metadata.states[1].has_data);
    assert!(metadata.states[1].fields.is_empty());
    assert_eq!(metadata.transitions[0].method_name, "submit");
    assert_eq!(metadata.transitions[0].from_state, "Draft");
    assert_eq!(metadata.transitions[0].to_states, vec!["Review"]);
}

#[test]
fn stable_metadata_emits_mermaid_state_diagram_from_graph_metadata() {
    let metadata = StableGraphMetadata::from_graph(&GRAPH);

    assert_eq!(
        metadata.to_mermaid_state_diagram(),
        concat!(
            "stateDiagram-v2\n",
            "    %% machine: review_flow::ReviewFlow\n",
            "    state \"Draft\" as s0\n",
            "    state \"Review\" as s1\n",
            "    state \"Published\" as s2\n",
            "    s0 --> s1: submit\n",
            "    s1 --> s2: publish\n",
        )
    );
}

#[test]
fn stable_metadata_mermaid_escapes_labels_and_keeps_unknown_targets_visible() {
    let mut metadata = StableGraphMetadata::from_graph(&GRAPH);
    metadata.machine.rust_type_path = "review_flow::ReviewFlow\nInjected".to_owned();
    metadata.states[0].label = Some("Draft \\\"quoted\\\"".to_owned());
    metadata.transitions[0].label = Some("submit \\\\ review\nnow".to_owned());
    metadata.transitions[1].to_states = vec!["Archived".to_owned()];

    assert_eq!(
        metadata.to_mermaid_state_diagram(),
        concat!(
            "stateDiagram-v2\n",
            "    %% machine: review_flow::ReviewFlow Injected\n",
            "    state \"Draft \\\\\\\"quoted\\\\\\\"\" as s0\n",
            "    state \"Review\" as s1\n",
            "    state \"Published\" as s2\n",
            "    s0 --> s1: submit \\\\ review now\n",
            "    s1 --> unknown_0: publish\n",
            "    state \"Archived\" as unknown_0\n",
        )
    );
}

#[test]
fn stable_metadata_emits_deterministic_dot_graph_from_graph_metadata() {
    let metadata = StableGraphMetadata::from_graph(&GRAPH);

    assert_eq!(
        metadata.to_dot_graph(),
        concat!(
            "digraph statum_workflow {\n",
            "    graph [label=\"review_flow::ReviewFlow\", labelloc=\"t\"];\n",
            "    node [shape=\"box\"];\n",
            "    s0 [label=\"Draft\"];\n",
            "    s1 [label=\"Review\"];\n",
            "    s2 [label=\"Published\"];\n",
            "    s0 -> s1 [label=\"submit\"];\n",
            "    s1 -> s2 [label=\"publish\"];\n",
            "}\n",
        )
    );
}

#[test]
fn stable_metadata_emits_allowed_forbidden_transition_matrix() {
    let metadata = StableGraphMetadata::from_graph(&GRAPH);

    assert_eq!(
        metadata.to_transition_matrix_table(),
        concat!(
            "| from \\ to | Draft | Review | Published |\n",
            "| --- | --- | --- | --- |\n",
            "| Draft | forbidden | submit | forbidden |\n",
            "| Review | forbidden | forbidden | publish |\n",
            "| Published | forbidden | forbidden | forbidden |\n",
        )
    );
}

#[test]
fn stable_metadata_dot_escapes_labels_and_keeps_unknown_targets_visible() {
    let mut metadata = StableGraphMetadata::from_graph(&GRAPH);
    metadata.machine.rust_type_path = "review_flow::ReviewFlow\nInjected".to_owned();
    metadata.states[0].label = Some("Draft \\\"quoted\\\"".to_owned());
    metadata.transitions[0].label = Some("submit \\\\ review\nnow".to_owned());
    metadata.transitions[1].to_states = vec!["Archived".to_owned()];

    assert_eq!(
        metadata.to_dot_graph(),
        concat!(
            "digraph statum_workflow {\n",
            "    graph [label=\"review_flow::ReviewFlow\\nInjected\", labelloc=\"t\"];\n",
            "    node [shape=\"box\"];\n",
            "    s0 [label=\"Draft \\\\\\\"quoted\\\\\\\"\"];\n",
            "    s1 [label=\"Review\"];\n",
            "    s2 [label=\"Published\"];\n",
            "    s0 -> s1 [label=\"submit \\\\\\\\ review\\nnow\"];\n",
            "    s1 -> unknown_0 [label=\"publish\"];\n",
            "    unknown_0 [label=\"Archived\"];\n",
            "}\n",
        )
    );
}

#[test]
fn stable_metadata_lints_terminal_state_with_outgoing_transition() {
    let metadata = StableGraphMetadata {
        transitions: vec![
            TRANSITIONS[0].into_stable_metadata(&GRAPH),
            TRANSITIONS[1].into_stable_metadata(&GRAPH),
            statum_core::StableTransitionMetadata {
                method_name: "unpublish".to_owned(),
                label: None,
                description: None,
                from_state: "Published".to_owned(),
                to_states: vec!["Review".to_owned()],
            },
        ],
        ..StableGraphMetadata::from_graph(&GRAPH)
    };

    let lints = metadata.lint_graph_invariants();

    assert_eq!(lints.len(), 1);
    assert_eq!(
        lints[0].code,
        GraphLintCode::TerminalStateHasOutgoingTransition
    );
    assert_eq!(lints[0].state.as_deref(), Some("Published"));
    assert_eq!(lints[0].transition.as_deref(), Some("unpublish"));
    assert!(lints[0].message.contains("Published"));
}

#[test]
fn stable_metadata_lint_summary_documents_authority_and_false_positive_boundary() {
    let mut metadata = StableGraphMetadata::from_graph(&GRAPH);
    metadata
        .transitions
        .push(statum_core::StableTransitionMetadata {
            method_name: "unpublish".to_owned(),
            label: None,
            description: None,
            from_state: "Published".to_owned(),
            to_states: vec!["Review".to_owned()],
        });

    assert_eq!(
        metadata.to_graph_lint_report(),
        concat!(
            "Graph invariant lint report for review_flow::ReviewFlow\n",
            "authority: cfg_pruned_macro_input\n",
            "false-positive boundary: lints inspect only StableGraphMetadata; ",
            "runtime-only policy, external guard conditions, and transition sites rejected before metadata emission are outside this report.\n",
            "\n",
            "- terminal_state_has_outgoing_transition: state `Published` has outgoing transition `unpublish`; terminal-state lint treats terminal-looking state names as terminal candidates.\n",
        )
    );
}

#[test]
fn stable_metadata_lint_report_states_when_no_invariants_trigger() {
    let metadata = StableGraphMetadata::from_graph(&GRAPH);

    assert_eq!(
        metadata.to_graph_lint_report(),
        concat!(
            "Graph invariant lint report for review_flow::ReviewFlow\n",
            "authority: cfg_pruned_macro_input\n",
            "false-positive boundary: lints inspect only StableGraphMetadata; ",
            "runtime-only policy, external guard conditions, and transition sites rejected before metadata emission are outside this report.\n",
            "\n",
            "No graph invariant warnings.\n",
        )
    );
}

#[test]
fn stable_metadata_lints_missing_unreachable_and_self_transition_smells() {
    let metadata = StableGraphMetadata {
        transitions: vec![statum_core::StableTransitionMetadata {
            method_name: "loop_review".to_owned(),
            label: None,
            description: None,
            from_state: "Review".to_owned(),
            to_states: vec!["Review".to_owned()],
        }],
        ..StableGraphMetadata::from_graph(&GRAPH)
    };

    let codes = metadata
        .lint_graph_invariants()
        .into_iter()
        .map(|finding| finding.code)
        .collect::<Vec<_>>();

    assert!(codes.contains(&GraphLintCode::SuspiciousSelfTransition));
    assert!(codes.contains(&GraphLintCode::MissingIncomingPath));
    assert!(codes.contains(&GraphLintCode::UnreachableState));
}

#[test]
fn stable_metadata_lint_report_escapes_text_fields_for_plain_text_output() {
    let mut metadata = StableGraphMetadata::from_graph(&GRAPH);
    metadata.machine.rust_type_path = "review_flow::ReviewFlow\nInjected\rTabbed\tValue".to_owned();
    metadata
        .transitions
        .push(statum_core::StableTransitionMetadata {
            method_name: "unpublish\nInjected\rTabbed\tValue".to_owned(),
            label: None,
            description: None,
            from_state: "Published".to_owned(),
            to_states: vec!["Review".to_owned()],
        });

    let report = metadata.to_graph_lint_report();

    assert!(
        report.contains("review_flow::ReviewFlow Injected Tabbed Value"),
        "{report}"
    );
    assert!(
        report.contains("unpublish Injected Tabbed Value"),
        "{report}"
    );
    assert!(!report.contains("\nInjected"), "{report}");
    assert!(!report.contains("\rTabbed"), "{report}");
    assert!(!report.contains("\tValue"), "{report}");
}

trait IntoStableTransitionMetadata {
    fn into_stable_metadata(
        self,
        graph: &MachineGraph<StateId, TransitionId>,
    ) -> statum_core::StableTransitionMetadata;
}

impl IntoStableTransitionMetadata for TransitionDescriptor<StateId, TransitionId> {
    fn into_stable_metadata(
        self,
        graph: &MachineGraph<StateId, TransitionId>,
    ) -> statum_core::StableTransitionMetadata {
        statum_core::StableTransitionMetadata {
            method_name: self.method_name.to_owned(),
            label: None,
            description: None,
            from_state: graph
                .state(self.from)
                .expect("source state exists")
                .rust_name
                .to_owned(),
            to_states: self
                .to
                .iter()
                .map(|target| {
                    graph
                        .state(*target)
                        .expect("target state exists")
                        .rust_name
                        .to_owned()
                })
                .collect(),
        }
    }
}

#[test]
fn stable_metadata_serializes_to_explicit_json_shape() {
    let metadata = StableGraphMetadata::from_graph(&GRAPH);
    let json = serde_json::to_value(&metadata).expect("stable metadata should serialize");
    let round_tripped: StableGraphMetadata =
        serde_json::from_value(json.clone()).expect("stable metadata should deserialize");

    assert_eq!(round_tripped, metadata);
    assert_eq!(json["version"], "v1");
    assert_eq!(json["authority"], "cfg_pruned_macro_input");
    assert_eq!(json["machine"]["module_path"], "review_flow");
    assert_eq!(json["machine"]["fields"], serde_json::json!([]));
    assert_eq!(json["states"][1]["rust_name"], "Review");
    assert_eq!(json["states"][1]["label"], serde_json::Value::Null);
    assert_eq!(json["states"][1]["fields"], serde_json::json!([]));
    assert_eq!(json["transitions"][0]["label"], serde_json::Value::Null);
    assert_eq!(json["transitions"][0]["from_state"], "Draft");
    assert_eq!(
        json["transitions"][0]["to_states"],
        serde_json::json!(["Review"])
    );
    assert_eq!(
        json["unsupported_cases"],
        serde_json::json!([
            "runtime_only_transitions",
            "cfg_ambiguous_aliases",
            "unexpanded_custom_decision_enums",
            "macro_generated_items",
            "include_generated_items",
            "field_level_presentation_metadata"
        ])
    );
}

#[test]
fn stable_metadata_renderers_use_positional_ids_under_duplicate_name_pressure() {
    let mut metadata = StableGraphMetadata::from_graph(&GRAPH);
    metadata.states[0].label = Some("Review".to_owned());
    metadata.states[1].label = Some("Review".to_owned());
    metadata.transitions[0].label = Some("advance".to_owned());
    metadata.transitions[1].label = Some("advance".to_owned());

    let mermaid = metadata.to_mermaid_state_diagram();
    assert!(mermaid.contains("state \"Review\" as s0"), "{mermaid}");
    assert!(mermaid.contains("state \"Review\" as s1"), "{mermaid}");
    assert!(mermaid.contains("s0 --> s1: advance"), "{mermaid}");
    assert!(mermaid.contains("s1 --> s2: advance"), "{mermaid}");

    let dot = metadata.to_dot_graph();
    assert!(dot.contains("s0 [label=\"Review\"]"), "{dot}");
    assert!(dot.contains("s1 [label=\"Review\"]"), "{dot}");
    assert!(dot.contains("s0 -> s1 [label=\"advance\"]"), "{dot}");
    assert!(dot.contains("s1 -> s2 [label=\"advance\"]"), "{dot}");
}
