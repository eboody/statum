use std::collections::{HashMap, HashSet};
use std::fmt;

use statum::{
    LinkedJourneyDescriptor, LinkedJourneyStepDescriptor, linked_journeys, linked_reference_types,
};
use statum_graph::{CodebaseDoc, CodebaseMachineRelationGroup};

use crate::heuristics::{HeuristicMachineRelationGroup, HeuristicOverlay};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct JourneyOverlay {
    journeys: Vec<ResolvedJourney>,
}

impl JourneyOverlay {
    pub(crate) fn journeys(&self) -> &[ResolvedJourney] {
        &self.journeys
    }

    pub(crate) fn journey(&self, index: usize) -> Option<&ResolvedJourney> {
        self.journeys.get(index)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.journeys.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedJourney {
    pub(crate) index: usize,
    pub(crate) full_id: String,
    pub(crate) module_path: &'static str,
    pub(crate) id: &'static str,
    pub(crate) label: Option<&'static str>,
    pub(crate) docs: Option<&'static str>,
    pub(crate) nodes: Vec<ResolvedJourneyNode>,
    pub(crate) segments: Vec<ResolvedJourneySegment>,
}

impl ResolvedJourney {
    pub(crate) fn display_label(&self) -> &str {
        self.label.unwrap_or(self.id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JourneyNodeRole {
    Entry,
    Step,
    Outcome,
}

impl JourneyNodeRole {
    pub(crate) fn display_label(self) -> &'static str {
        match self {
            Self::Entry => "entry",
            Self::Step => "step",
            Self::Outcome => "outcome",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JourneyBridgeTarget {
    pub(crate) machine: usize,
    pub(crate) state: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum JourneyNodeReference {
    Machine {
        machine: usize,
    },
    State {
        machine: usize,
        state: usize,
    },
    Validator {
        machine: usize,
        entry: usize,
        source_type_display: &'static str,
    },
    Bridge {
        type_display: &'static str,
        resolved_type_name: &'static str,
        declared_reference_target: Option<JourneyBridgeTarget>,
    },
}

impl JourneyNodeReference {
    pub(crate) fn machine(&self) -> Option<usize> {
        match self {
            Self::Machine { machine }
            | Self::State { machine, .. }
            | Self::Validator { machine, .. } => Some(*machine),
            Self::Bridge { .. } => None,
        }
    }

    pub(crate) fn is_bridge(&self) -> bool {
        matches!(self, Self::Bridge { .. })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedJourneyNode {
    pub(crate) index: usize,
    pub(crate) role: JourneyNodeRole,
    pub(crate) reference: JourneyNodeReference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JourneySegmentKind {
    Exact,
    DeclaredBridge,
    HeuristicCover,
    Missing,
}

impl JourneySegmentKind {
    pub(crate) fn display_label(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::DeclaredBridge => "declared bridge",
            Self::HeuristicCover => "heuristic cover",
            Self::Missing => "missing",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JourneySegmentBasis {
    SameMachine,
    CompositionMachineRelation,
    ExactMachineRelation,
    DeclaredBridge,
    HeuristicMachineRelation,
    Missing,
}

impl JourneySegmentBasis {
    pub(crate) fn display_label(self) -> &'static str {
        match self {
            Self::SameMachine => "same machine",
            Self::CompositionMachineRelation => "composition machine relation",
            Self::ExactMachineRelation => "exact machine relation",
            Self::DeclaredBridge => "declared bridge",
            Self::HeuristicMachineRelation => "heuristic machine relation",
            Self::Missing => "missing",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedJourneySegment {
    pub(crate) index: usize,
    pub(crate) from_node: usize,
    pub(crate) to_node: usize,
    pub(crate) from_machine: Option<usize>,
    pub(crate) to_machine: Option<usize>,
    pub(crate) declared_bridge: bool,
    pub(crate) same_machine: bool,
    pub(crate) exact_is_composition_owned: bool,
    pub(crate) exact_label: Option<String>,
    pub(crate) exact_count: usize,
    pub(crate) heuristic_label: Option<String>,
    pub(crate) heuristic_count: usize,
}

impl ResolvedJourneySegment {
    pub(crate) fn visible_kind(&self, shows_heuristic: bool) -> JourneySegmentKind {
        if self.declared_bridge {
            JourneySegmentKind::DeclaredBridge
        } else if self.same_machine || self.exact_count > 0 {
            JourneySegmentKind::Exact
        } else if shows_heuristic && self.heuristic_count > 0 {
            JourneySegmentKind::HeuristicCover
        } else {
            JourneySegmentKind::Missing
        }
    }

    pub(crate) fn visible_basis(&self, shows_heuristic: bool) -> JourneySegmentBasis {
        if self.declared_bridge {
            JourneySegmentBasis::DeclaredBridge
        } else if self.same_machine {
            JourneySegmentBasis::SameMachine
        } else if self.exact_count > 0 && self.exact_is_composition_owned {
            JourneySegmentBasis::CompositionMachineRelation
        } else if self.exact_count > 0 {
            JourneySegmentBasis::ExactMachineRelation
        } else if shows_heuristic && self.heuristic_count > 0 {
            JourneySegmentBasis::HeuristicMachineRelation
        } else {
            JourneySegmentBasis::Missing
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct JourneySegmentCounts {
    pub(crate) exact: usize,
    pub(crate) declared: usize,
    pub(crate) heuristic: usize,
    pub(crate) missing: usize,
}

#[derive(Debug)]
pub(crate) enum JourneyOverlayError {
    DuplicateJourney {
        full_id: String,
    },
    MissingMachine {
        journey: String,
        machine_path: String,
    },
    MissingState {
        journey: String,
        machine_path: String,
        state: &'static str,
    },
    MissingValidator {
        journey: String,
        machine_path: String,
        source_type_display: &'static str,
    },
    AmbiguousValidator {
        journey: String,
        machine_path: String,
        source_type_display: &'static str,
    },
    DuplicateReferenceType {
        resolved_type_name: &'static str,
    },
    MissingReferenceTargetMachine {
        type_display: &'static str,
        target_machine_path: String,
    },
    MissingReferenceTargetState {
        type_display: &'static str,
        target_machine_path: String,
        target_state: &'static str,
    },
}

impl fmt::Display for JourneyOverlayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateJourney { full_id } => {
                write!(formatter, "journey `{full_id}` was declared more than once")
            }
            Self::MissingMachine {
                journey,
                machine_path,
            } => write!(
                formatter,
                "journey `{journey}` points at missing machine `{machine_path}`"
            ),
            Self::MissingState {
                journey,
                machine_path,
                state,
            } => write!(
                formatter,
                "journey `{journey}` points at missing state `{machine_path}::{state}`"
            ),
            Self::MissingValidator {
                journey,
                machine_path,
                source_type_display,
            } => write!(
                formatter,
                "journey `{journey}` points at missing validator `{source_type_display}::into_machine()` for machine `{machine_path}`"
            ),
            Self::AmbiguousValidator {
                journey,
                machine_path,
                source_type_display,
            } => write!(
                formatter,
                "journey `{journey}` points at ambiguous validator `{source_type_display}::into_machine()` for machine `{machine_path}`"
            ),
            Self::DuplicateReferenceType { resolved_type_name } => write!(
                formatter,
                "nominal bridge type `{resolved_type_name}` appears more than once in the linked reference-type inventory"
            ),
            Self::MissingReferenceTargetMachine {
                type_display,
                target_machine_path,
            } => write!(
                formatter,
                "bridge type `{type_display}` declares missing machine-ref target `{target_machine_path}`"
            ),
            Self::MissingReferenceTargetState {
                type_display,
                target_machine_path,
                target_state,
            } => write!(
                formatter,
                "bridge type `{type_display}` declares missing machine-ref target state `{target_machine_path}::{target_state}`"
            ),
        }
    }
}

impl std::error::Error for JourneyOverlayError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolvedReferenceTypeTarget {
    machine: usize,
    state: usize,
}

pub(crate) fn collect_journey_overlay(
    doc: &CodebaseDoc,
    heuristic: &HeuristicOverlay,
) -> Result<JourneyOverlay, JourneyOverlayError> {
    let exact_groups = doc
        .machine_relation_groups()
        .into_iter()
        .map(|group| ((group.from_machine, group.to_machine), group))
        .collect::<HashMap<_, _>>();
    let heuristic_groups = heuristic
        .machine_relation_groups()
        .into_iter()
        .map(|group| ((group.from_machine, group.to_machine), group))
        .collect::<HashMap<_, _>>();
    let reference_targets = resolve_reference_type_targets(doc)?;

    let mut linked = linked_journeys().to_vec();
    linked.sort_by(|left, right| {
        left.module_path
            .cmp(right.module_path)
            .then_with(|| left.id.cmp(right.id))
    });

    let mut seen = HashSet::with_capacity(linked.len());
    let mut journeys = Vec::with_capacity(linked.len());

    for linked_journey in linked {
        let full_id = format!("{}::{}", linked_journey.module_path, linked_journey.id);
        if !seen.insert(full_id.clone()) {
            return Err(JourneyOverlayError::DuplicateJourney { full_id });
        }

        let mut nodes = Vec::with_capacity(linked_journey.steps.len() + 2);
        nodes.push(resolve_step_node(
            doc,
            &linked_journey,
            &reference_targets,
            JourneyNodeRole::Entry,
            0,
            linked_journey.entry,
        )?);
        for (index, step) in linked_journey.steps.iter().copied().enumerate() {
            nodes.push(resolve_step_node(
                doc,
                &linked_journey,
                &reference_targets,
                JourneyNodeRole::Step,
                index + 1,
                step,
            )?);
        }
        nodes.push(resolve_step_node(
            doc,
            &linked_journey,
            &reference_targets,
            JourneyNodeRole::Outcome,
            nodes.len(),
            linked_journey.outcome,
        )?);

        let segments = nodes
            .windows(2)
            .enumerate()
            .map(|(index, window)| {
                build_segment(index, &window[0], &window[1], &exact_groups, &heuristic_groups)
            })
            .collect();

        journeys.push(ResolvedJourney {
            index: journeys.len(),
            full_id,
            module_path: linked_journey.module_path,
            id: linked_journey.id,
            label: linked_journey.label,
            docs: linked_journey.docs,
            nodes,
            segments,
        });
    }

    Ok(JourneyOverlay { journeys })
}

fn resolve_reference_type_targets(
    doc: &CodebaseDoc,
) -> Result<HashMap<&'static str, ResolvedReferenceTypeTarget>, JourneyOverlayError> {
    let mut resolved = HashMap::new();
    for descriptor in linked_reference_types() {
        let resolved_type_name = (descriptor.resolved_type_name)();
        if resolved.contains_key(resolved_type_name) {
            return Err(JourneyOverlayError::DuplicateReferenceType {
                resolved_type_name,
            });
        }
        let target_machine_path = descriptor.to_machine_path.join("::");
        let machine = doc
            .machines()
            .iter()
            .find(|machine| machine.rust_type_path == target_machine_path)
            .ok_or_else(|| JourneyOverlayError::MissingReferenceTargetMachine {
                type_display: descriptor.rust_type_path,
                target_machine_path: target_machine_path.clone(),
            })?;
        let state = machine.state_named(descriptor.to_state).ok_or_else(|| {
            JourneyOverlayError::MissingReferenceTargetState {
                type_display: descriptor.rust_type_path,
                target_machine_path: target_machine_path.clone(),
                target_state: descriptor.to_state,
            }
        })?;
        resolved.insert(
            resolved_type_name,
            ResolvedReferenceTypeTarget {
                machine: machine.index,
                state: state.index,
            },
        );
    }
    Ok(resolved)
}

fn resolve_step_node(
    doc: &CodebaseDoc,
    journey: &LinkedJourneyDescriptor,
    reference_targets: &HashMap<&'static str, ResolvedReferenceTypeTarget>,
    role: JourneyNodeRole,
    index: usize,
    step: LinkedJourneyStepDescriptor,
) -> Result<ResolvedJourneyNode, JourneyOverlayError> {
    let journey_id = format!("{}::{}", journey.module_path, journey.id);
    let reference = match step {
        LinkedJourneyStepDescriptor::Machine { machine_path } => {
            let machine = resolve_machine(doc, &journey_id, machine_path)?;
            JourneyNodeReference::Machine { machine }
        }
        LinkedJourneyStepDescriptor::State {
            machine_path,
            state,
        } => {
            let machine = resolve_machine(doc, &journey_id, machine_path)?;
            let machine_doc = doc
                .machine(machine)
                .expect("resolved machine index should exist");
            let state_doc = machine_doc.state_named(state).ok_or_else(|| {
                JourneyOverlayError::MissingState {
                    journey: journey_id.clone(),
                    machine_path: machine_path.join("::"),
                    state,
                }
            })?;
            JourneyNodeReference::State {
                machine,
                state: state_doc.index,
            }
        }
        LinkedJourneyStepDescriptor::Validator {
            source_type_display,
            resolved_source_type_name,
            machine_path,
        } => {
            let machine = resolve_machine(doc, &journey_id, machine_path)?;
            let machine_doc = doc
                .machine(machine)
                .expect("resolved machine index should exist");
            let matches = machine_doc
                .validator_entries
                .iter()
                .filter(|entry| {
                    entry.resolved_source_type_name == resolved_source_type_name()
                })
                .collect::<Vec<_>>();
            let [entry] = matches.as_slice() else {
                return Err(if matches.is_empty() {
                    JourneyOverlayError::MissingValidator {
                        journey: journey_id.clone(),
                        machine_path: machine_path.join("::"),
                        source_type_display,
                    }
                } else {
                    JourneyOverlayError::AmbiguousValidator {
                        journey: journey_id.clone(),
                        machine_path: machine_path.join("::"),
                        source_type_display,
                    }
                });
            };
            JourneyNodeReference::Validator {
                machine,
                entry: entry.index,
                source_type_display,
            }
        }
        LinkedJourneyStepDescriptor::Bridge {
            type_display,
            resolved_type_name,
        } => JourneyNodeReference::Bridge {
            type_display,
            resolved_type_name: resolved_type_name(),
            declared_reference_target: reference_targets
                .get(resolved_type_name())
                .copied()
                .map(|target| JourneyBridgeTarget {
                    machine: target.machine,
                    state: target.state,
                }),
        },
    };

    Ok(ResolvedJourneyNode {
        index,
        role,
        reference,
    })
}

fn resolve_machine(
    doc: &CodebaseDoc,
    journey_id: &str,
    machine_path: &'static [&'static str],
) -> Result<usize, JourneyOverlayError> {
    let machine_path = machine_path.join("::");
    doc.machines()
        .iter()
        .find(|machine| machine.rust_type_path == machine_path)
        .map(|machine| machine.index)
        .ok_or_else(|| JourneyOverlayError::MissingMachine {
            journey: journey_id.to_owned(),
            machine_path,
        })
}

fn build_segment(
    index: usize,
    from: &ResolvedJourneyNode,
    to: &ResolvedJourneyNode,
    exact_groups: &HashMap<(usize, usize), CodebaseMachineRelationGroup>,
    heuristic_groups: &HashMap<(usize, usize), HeuristicMachineRelationGroup>,
) -> ResolvedJourneySegment {
    let from_machine = from.reference.machine();
    let to_machine = to.reference.machine();
    let declared_bridge = from.reference.is_bridge() || to.reference.is_bridge();
    let same_machine =
        matches!((from_machine, to_machine), (Some(left), Some(right)) if left == right);

    let (exact_count, exact_label, exact_is_composition_owned) = match (from_machine, to_machine) {
        (Some(from_machine), Some(to_machine)) if !declared_bridge => exact_groups
            .get(&(from_machine, to_machine))
            .map(|group| {
                (
                    group.relation_indices.len(),
                    Some(group.display_label()),
                    group.is_composition_owned(),
                )
            })
            .unwrap_or((0, None, false)),
        _ => (0, None, false),
    };
    let (heuristic_count, heuristic_label) = match (from_machine, to_machine) {
        (Some(from_machine), Some(to_machine)) if !declared_bridge => heuristic_groups
            .get(&(from_machine, to_machine))
            .map(|group| (group.relation_indices.len(), Some(group.display_label())))
            .unwrap_or((0, None)),
        _ => (0, None),
    };

    ResolvedJourneySegment {
        index,
        from_node: from.index,
        to_node: to.index,
        from_machine,
        to_machine,
        declared_bridge,
        same_machine,
        exact_is_composition_owned,
        exact_label,
        exact_count,
        heuristic_label,
        heuristic_count,
    }
}

pub(crate) fn visible_journey_counts(
    journey: &ResolvedJourney,
    shows_heuristic: bool,
) -> JourneySegmentCounts {
    let mut counts = JourneySegmentCounts::default();
    for segment in &journey.segments {
        match segment.visible_kind(shows_heuristic) {
            JourneySegmentKind::Exact => counts.exact += 1,
            JourneySegmentKind::DeclaredBridge => counts.declared += 1,
            JourneySegmentKind::HeuristicCover => counts.heuristic += 1,
            JourneySegmentKind::Missing => counts.missing += 1,
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    use statum::{journeys, machine, machine_ref, state, transition, validators, Error};

    #[allow(dead_code)]
    mod journey_task {
        use super::*;

        #[state]
        pub enum State {
            Idle,
            Running,
            Done,
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

    #[allow(dead_code)]
    mod journey_workflow {
        use super::*;

        #[state]
        pub enum State {
            Draft,
            InProgress(super::journey_task::Machine<super::journey_task::Running>),
            Done,
        }

        #[machine]
        pub struct Machine<State> {}

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

            fn is_in_progress(
                &self,
            ) -> statum::Result<super::journey_task::Machine<super::journey_task::Running>> {
                if self.status == "running" {
                    Ok(
                        super::journey_task::Machine::<super::journey_task::Running>::builder()
                            .build(),
                    )
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

        #[machine_ref(super::journey_task::Machine<super::journey_task::Running>)]
        pub struct TaskReceipt(pub u64);
    }

    journeys! {
        journey workflow_story {
            label: "Workflow Story";
            docs: "Explains workflow start, handoff, and task execution.";
            entry: validator!(self::journey_workflow::WorkflowRow => self::journey_workflow::Machine);
            steps: [
                state!(self::journey_workflow::Machine, InProgress),
                bridge!(self::journey_workflow::TaskReceipt),
                machine!(self::journey_task::Machine)
            ];
            outcome: state!(self::journey_task::Machine, Running);
        }
    }

    fn fixture_doc() -> CodebaseDoc {
        CodebaseDoc::linked().expect("linked codebase doc")
    }

    fn empty_heuristic_overlay() -> HeuristicOverlay {
        HeuristicOverlay::from_parts(crate::HeuristicStatusKind::Available, Vec::new(), Vec::new())
    }

    #[test]
    fn resolves_declared_journey_overlay() {
        let doc = fixture_doc();
        let overlay = collect_journey_overlay(&doc, &empty_heuristic_overlay())
            .expect("journey overlay");

        assert_eq!(overlay.journeys().len(), 1);
        let journey = overlay.journey(0).expect("journey");
        assert_eq!(journey.display_label(), "Workflow Story");
        assert_eq!(journey.nodes.len(), 5);
        assert_eq!(journey.segments.len(), 4);

        assert!(matches!(
            journey.nodes[0].reference,
            JourneyNodeReference::Validator { .. }
        ));
        assert!(matches!(
            journey.nodes[1].reference,
            JourneyNodeReference::State { .. }
        ));
        assert!(matches!(
            journey.nodes[2].reference,
            JourneyNodeReference::Bridge {
                declared_reference_target: Some(_),
                ..
            }
        ));
        assert!(matches!(
            journey.nodes[3].reference,
            JourneyNodeReference::Machine { .. }
        ));
        assert!(matches!(
            journey.nodes[4].reference,
            JourneyNodeReference::State { .. }
        ));

        let counts = visible_journey_counts(journey, false);
        assert_eq!(counts.exact, 2);
        assert_eq!(counts.declared, 2);
        assert_eq!(counts.heuristic, 0);
        assert_eq!(counts.missing, 0);
    }
}
