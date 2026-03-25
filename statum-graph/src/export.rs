use std::borrow::Cow;

use serde::Serialize;
use statum::MachinePresentation;

use crate::MachineDoc;

/// Stable export model for one validated machine graph.
///
/// This type is the canonical renderer input for Mermaid, DOT, PlantUML, and
/// JSON output. Structure comes from [`statum::MachineIntrospection::GRAPH`];
/// labels and descriptions may be joined from a matching
/// [`statum::MachinePresentation`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExportDoc {
    /// Exported machine metadata.
    machine: ExportMachine,
    /// Exported states in stable graph order.
    states: Vec<ExportState>,
    /// Exported transition sites in stable graph order.
    transitions: Vec<ExportTransition>,
}

impl ExportDoc {
    /// Exported machine metadata.
    pub fn machine(&self) -> ExportMachine {
        self.machine
    }

    /// Exported states in stable graph order.
    pub fn states(&self) -> &[ExportState] {
        &self.states
    }

    /// Exported transition sites in stable graph order.
    pub fn transitions(&self) -> &[ExportTransition] {
        &self.transitions
    }

    /// Returns one exported state by its stable state index.
    pub fn state(&self, index: usize) -> Option<&ExportState> {
        self.states.get(index)
    }

    /// Returns one exported transition site by its stable transition index.
    pub fn transition(&self, index: usize) -> Option<&ExportTransition> {
        self.transitions.get(index)
    }
}

/// Machine metadata preserved in the stable export surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ExportMachine {
    /// `module_path!()` for the source module that owns the machine.
    pub module_path: &'static str,
    /// Fully qualified Rust type path for the machine family.
    pub rust_type_path: &'static str,
    /// Optional human-facing label from presentation metadata.
    pub label: Option<&'static str>,
    /// Optional human-facing description from presentation metadata.
    pub description: Option<&'static str>,
}

/// State metadata preserved in the stable export surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ExportState {
    /// Stable export-local state index.
    pub index: usize,
    /// Rust variant name emitted by Statum.
    pub rust_name: &'static str,
    /// Optional human-facing label from presentation metadata.
    pub label: Option<&'static str>,
    /// Optional human-facing description from presentation metadata.
    pub description: Option<&'static str>,
    /// Whether the state carries `state_data`.
    pub has_data: bool,
    /// Whether the state has no incoming edge in the exported topology.
    pub is_root: bool,
}

impl ExportState {
    /// Stable renderer node id for this state.
    pub fn node_id(&self) -> String {
        format!("s{}", self.index)
    }

    /// Human-facing state label used by text renderers.
    pub fn display_label(&self) -> Cow<'static, str> {
        match self.label {
            Some(label) => Cow::Borrowed(label),
            None if self.has_data => Cow::Owned(format!("{} (data)", self.rust_name)),
            None => Cow::Borrowed(self.rust_name),
        }
    }
}

/// Transition-site metadata preserved in the stable export surface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExportTransition {
    /// Stable export-local transition index.
    pub index: usize,
    /// Rust method name emitted by Statum.
    pub method_name: &'static str,
    /// Optional human-facing label from presentation metadata.
    pub label: Option<&'static str>,
    /// Optional human-facing description from presentation metadata.
    pub description: Option<&'static str>,
    /// Stable source-state index.
    pub from: usize,
    /// Stable legal target-state indices for this transition site.
    pub to: Vec<usize>,
}

impl ExportTransition {
    /// Stable renderer transition id for this transition site.
    pub fn transition_id(&self) -> String {
        format!("t{}", self.index)
    }

    /// Human-facing edge label used by text renderers.
    pub fn display_label(&self) -> &'static str {
        self.label.unwrap_or(self.method_name)
    }
}

/// Error returned when presentation metadata cannot be joined onto a
/// validated machine graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportDocError {
    /// One state presentation entry points at a state id that is not in the
    /// validated graph.
    UnknownStatePresentation { machine: &'static str, entry: usize },
    /// One state id appears more than once in the presentation overlay.
    DuplicateStatePresentation { machine: &'static str, entry: usize },
    /// One transition presentation entry points at a transition id that is not
    /// in the validated graph.
    UnknownTransitionPresentation { machine: &'static str, entry: usize },
    /// One transition id appears more than once in the presentation overlay.
    DuplicateTransitionPresentation { machine: &'static str, entry: usize },
}

impl core::fmt::Display for ExportDocError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownStatePresentation { machine, entry } => write!(
                formatter,
                "presentation for machine `{machine}` contains state entry {} whose id is missing from the graph",
                entry + 1
            ),
            Self::DuplicateStatePresentation { machine, entry } => write!(
                formatter,
                "presentation for machine `{machine}` contains duplicate state id at entry {}",
                entry + 1
            ),
            Self::UnknownTransitionPresentation { machine, entry } => write!(
                formatter,
                "presentation for machine `{machine}` contains transition entry {} whose id is missing from the graph",
                entry + 1
            ),
            Self::DuplicateTransitionPresentation { machine, entry } => write!(
                formatter,
                "presentation for machine `{machine}` contains duplicate transition id at entry {}",
                entry + 1
            ),
        }
    }
}

impl std::error::Error for ExportDocError {}

impl<S, T> From<&MachineDoc<S, T>> for ExportDoc
where
    S: Eq,
{
    fn from(doc: &MachineDoc<S, T>) -> Self {
        Self {
            machine: ExportMachine {
                module_path: doc.machine().module_path,
                rust_type_path: doc.machine().rust_type_path,
                label: None,
                description: None,
            },
            states: doc
                .states()
                .iter()
                .enumerate()
                .map(|(index, state)| ExportState {
                    index,
                    rust_name: state.descriptor.rust_name,
                    label: None,
                    description: None,
                    has_data: state.descriptor.has_data,
                    is_root: state.is_root,
                })
                .collect(),
            transitions: doc
                .edges()
                .iter()
                .enumerate()
                .map(|(index, edge)| ExportTransition {
                    index,
                    method_name: edge.descriptor.method_name,
                    label: None,
                    description: None,
                    from: doc
                        .states()
                        .iter()
                        .position(|state| state.descriptor.id == edge.descriptor.from)
                        .expect("MachineDoc state ids should align with edges"),
                    to: edge
                        .descriptor
                        .to
                        .iter()
                        .map(|target| {
                            doc.states()
                                .iter()
                                .position(|state| state.descriptor.id == *target)
                                .expect("MachineDoc target ids should align with states")
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

pub trait ExportSource {
    fn export_doc(&self) -> Cow<'_, ExportDoc>;
}

impl ExportSource for ExportDoc {
    fn export_doc(&self) -> Cow<'_, ExportDoc> {
        Cow::Borrowed(self)
    }
}

impl<S, T> ExportSource for MachineDoc<S, T>
where
    S: Eq,
{
    fn export_doc(&self) -> Cow<'_, ExportDoc> {
        Cow::Owned(self.export())
    }
}

impl<S, T> MachineDoc<S, T>
where
    S: Eq,
{
    /// Builds the canonical stable export model without presentation metadata.
    pub fn export(&self) -> ExportDoc {
        ExportDoc::from(self)
    }
}

impl<S, T> MachineDoc<S, T>
where
    S: Copy + Eq + 'static,
    T: Copy + Eq + 'static,
{
    /// Builds the canonical stable export model and joins matching labels and
    /// descriptions from a presentation overlay.
    pub fn export_with_presentation<MachineMeta, StateMeta, TransitionMeta>(
        &self,
        presentation: &MachinePresentation<S, T, MachineMeta, StateMeta, TransitionMeta>,
    ) -> Result<ExportDoc, ExportDocError> {
        let mut export = self.export();

        if let Some(machine) = &presentation.machine {
            export.machine.label = machine.label;
            export.machine.description = machine.description;
        }

        for (entry, presented_state) in presentation.states.iter().enumerate() {
            let Some(index) = self
                .states()
                .iter()
                .position(|state| state.descriptor.id == presented_state.id)
            else {
                return Err(ExportDocError::UnknownStatePresentation {
                    machine: self.machine().rust_type_path,
                    entry,
                });
            };

            let export_state = &mut export.states[index];
            if export_state.label.is_some() || export_state.description.is_some() {
                return Err(ExportDocError::DuplicateStatePresentation {
                    machine: self.machine().rust_type_path,
                    entry,
                });
            }

            export_state.label = presented_state.label;
            export_state.description = presented_state.description;
        }

        for (entry, presented_transition) in presentation.transitions.iter().enumerate() {
            let Some(index) = self
                .edges()
                .iter()
                .position(|edge| edge.descriptor.id == presented_transition.id)
            else {
                return Err(ExportDocError::UnknownTransitionPresentation {
                    machine: self.machine().rust_type_path,
                    entry,
                });
            };

            let export_transition = &mut export.transitions[index];
            if export_transition.label.is_some() || export_transition.description.is_some() {
                return Err(ExportDocError::DuplicateTransitionPresentation {
                    machine: self.machine().rust_type_path,
                    entry,
                });
            }

            export_transition.label = presented_transition.label;
            export_transition.description = presented_transition.description;
        }

        Ok(export)
    }
}
