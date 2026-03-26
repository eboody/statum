#![allow(dead_code)]

use statum_graph::{
    CodebaseDoc, CodebaseRelationBasis, CodebaseRelationKind, CodebaseRelationSource,
};

mod task {
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Idle,
        Running,
    }

    #[machine]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Idle> {
        fn start(self) -> Machine<Running> {
            self.transition()
        }
    }
}

mod shadowed {
    pub struct Option<T>(pub T);
}

mod workflow {
    use super::{shadowed, task};
    use statum::{machine, state, transition};

    #[state]
    pub enum State {
        Draft,
        InProgress(super::task::Machine<super::task::Running>),
        Done,
    }

    #[machine]
    pub struct Machine<State> {
        current: ::core::option::Option<super::task::Machine<super::task::Running>>,
        ambiguous_current: task::Machine<task::Running>,
        shadowed_current: shadowed::Option<super::task::Machine<super::task::Running>>,
    }

    #[transition]
    impl Machine<Draft> {
        fn start(
            self,
            task: super::task::Machine<super::task::Running>,
            ambiguous: task::Machine<task::Running>,
            shadowed: shadowed::Option<super::task::Machine<super::task::Running>>,
        ) -> Machine<InProgress> {
            let _ = ambiguous;
            let _ = shadowed.0;
            self.transition_with(task)
        }
    }

    #[transition]
    impl Machine<InProgress> {
        fn finish(self) -> Machine<Done> {
            self.transition()
        }
    }
}

mod opaque {
    use statum::{machine, machine_ref, state, transition};

    #[machine_ref(super::task::Machine<super::task::Running>)]
    pub struct TaskId(u64);

    pub struct PlainTaskId(u64);

    #[state]
    pub enum State {
        Draft { child: TaskId, plain: PlainTaskId },
        Ready,
        Done,
    }

    #[machine]
    pub struct Machine<State> {
        selected: ::core::option::Option<TaskId>,
        ignored: ::core::option::Option<PlainTaskId>,
    }

    #[transition]
    impl Machine<Draft> {
        fn attach(self, task: TaskId, ignored: PlainTaskId) -> Machine<Ready> {
            let _ = task.0;
            let _ = ignored.0;
            self.transition()
        }
    }

    #[transition]
    impl Machine<Ready> {
        fn finish_generic<S: super::task::StateTrait>(
            self,
            task: super::task::Machine<S>,
        ) -> Machine<Done> {
            let _ = ::core::any::type_name::<S>();
            let _ = task;
            self.transition()
        }

        fn finish(self) -> Machine<Done> {
            self.transition()
        }
    }
}

#[test]
fn linked_codebase_exports_exact_relations_and_builder_metadata() {
    let doc = CodebaseDoc::linked().expect("linked codebase doc");

    let task = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("task::Machine"))
        .expect("task machine");
    let workflow = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("workflow::Machine"))
        .expect("workflow machine");
    let opaque = doc
        .machines()
        .iter()
        .find(|machine| machine.rust_type_path.ends_with("opaque::Machine"))
        .expect("opaque machine");
    let running_state = task
        .states
        .iter()
        .find(|state| state.rust_name == "Running")
        .expect("running state");

    assert!(workflow
        .states
        .iter()
        .all(|state| state.direct_construction_available));
    assert!(opaque
        .states
        .iter()
        .all(|state| state.direct_construction_available));

    assert_eq!(doc.links().len(), 1);
    assert_eq!(doc.relations().len(), 6);

    let workflow_transition = workflow
        .transitions
        .iter()
        .find(|transition| transition.method_name == "start")
        .expect("workflow start transition");
    let workflow_relation = doc
        .relations()
        .iter()
        .find(|relation| {
            relation.kind == CodebaseRelationKind::TransitionParam
                && relation.basis == CodebaseRelationBasis::DirectTypeSyntax
                && matches!(
                    relation.source,
                    CodebaseRelationSource::TransitionParam {
                        machine,
                        transition,
                        param_index: 0,
                        param_name: Some("task"),
                    } if machine == workflow.index && transition == workflow_transition.index
                )
        })
        .expect("workflow transition relation");
    assert_eq!(workflow_relation.target_machine, task.index);
    assert_eq!(workflow_relation.target_state, running_state.index);
    assert_eq!(workflow_relation.declared_reference_type, None);

    let workflow_field_relation = doc
        .relations()
        .iter()
        .find(|relation| {
            relation.kind == CodebaseRelationKind::MachineField
                && relation.basis == CodebaseRelationBasis::DirectTypeSyntax
                && matches!(
                    relation.source,
                    CodebaseRelationSource::MachineField {
                        machine,
                        field_name: Some("current"),
                        field_index: 0,
                    } if machine == workflow.index
                )
        })
        .expect("workflow machine field relation");
    assert_eq!(workflow_field_relation.target_machine, task.index);
    assert_eq!(workflow_field_relation.target_state, running_state.index);
    assert!(
        doc.relations().iter().all(|relation| {
            !matches!(
                relation.source,
                CodebaseRelationSource::MachineField {
                    machine,
                    field_name: Some("ambiguous_current" | "shadowed_current"),
                    ..
                } if machine == workflow.index
            )
        }),
        "ambiguous direct-machine syntax and same-name wrapper lookalikes should not create exact machine field relations"
    );

    let opaque_state_relation = doc
        .relations()
        .iter()
        .find(|relation| {
            relation.kind == CodebaseRelationKind::StatePayload
                && relation.basis == CodebaseRelationBasis::DeclaredReferenceType
                && matches!(
                    relation.source,
                    CodebaseRelationSource::StatePayload {
                        machine,
                        field_name: Some("child"),
                        ..
                    } if machine == opaque.index
                )
        })
        .expect("opaque state payload relation");
    assert_eq!(opaque_state_relation.target_machine, task.index);
    assert_eq!(opaque_state_relation.target_state, running_state.index);
    assert!(opaque_state_relation
        .declared_reference_type
        .is_some_and(|path| path.ends_with("opaque::TaskId")));

    let opaque_field_relation = doc
        .relations()
        .iter()
        .find(|relation| {
            relation.kind == CodebaseRelationKind::MachineField
                && relation.basis == CodebaseRelationBasis::DeclaredReferenceType
                && matches!(
                    relation.source,
                    CodebaseRelationSource::MachineField {
                        machine,
                        field_name: Some("selected"),
                        field_index: 0,
                    } if machine == opaque.index
                )
        })
        .expect("opaque machine field relation");
    assert_eq!(opaque_field_relation.target_machine, task.index);
    assert_eq!(opaque_field_relation.target_state, running_state.index);

    let opaque_transition = opaque
        .transitions
        .iter()
        .find(|transition| transition.method_name == "attach")
        .expect("opaque attach transition");
    let opaque_transition_relation = doc
        .relations()
        .iter()
        .find(|relation| {
            relation.kind == CodebaseRelationKind::TransitionParam
                && relation.basis == CodebaseRelationBasis::DeclaredReferenceType
                && matches!(
                    relation.source,
                    CodebaseRelationSource::TransitionParam {
                        machine,
                        transition,
                        param_index: 0,
                        param_name: Some("task"),
                    } if machine == opaque.index && transition == opaque_transition.index
                )
        })
        .expect("opaque transition relation");
    assert_eq!(opaque_transition_relation.target_machine, task.index);
    assert_eq!(opaque_transition_relation.target_state, running_state.index);

    let opaque_generic_transition = opaque
        .transitions
        .iter()
        .find(|transition| transition.method_name == "finish_generic")
        .expect("opaque generic transition");
    assert!(
        doc.relations().iter().all(|relation| {
            !matches!(
                relation.source,
                CodebaseRelationSource::TransitionParam {
                    machine,
                    transition,
                    ..
                } if machine == opaque.index && transition == opaque_generic_transition.index
            )
        }),
        "generic state parameters should not create exact transition relations"
    );

    assert!(
        doc.relations().iter().all(|relation| {
            !matches!(
                relation.source,
                CodebaseRelationSource::StatePayload {
                    machine,
                    field_name: Some("plain"),
                    ..
                } if machine == opaque.index
            )
        }),
        "plain opaque ids without #[machine_ref(...)] should not create exact relations"
    );
}
