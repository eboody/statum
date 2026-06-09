use crate::MachineGraph;
use std::vec::IntoIter;

/// One graph-metadata step in a legal walk.
///
/// The step records the transition site and the chosen target. For branching
/// transitions, `transition`, `method_name`, and `legal_targets` stay the same
/// while `chosen_target` differs across generated walks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WalkStep<S: 'static, T: 'static> {
    /// Source state for this step.
    pub from: S,
    /// Transition-site identifier emitted by the machine metadata.
    pub transition: T,
    /// Rust method name for the transition site.
    pub method_name: &'static str,
    /// All graph-declared legal targets for the transition site.
    pub legal_targets: &'static [S],
    /// Target selected for this particular walk branch.
    pub chosen_target: S,
}

/// A finite path through graph metadata.
///
/// Legal walks are metadata-only. They do not execute transition methods or prove
/// anything about method bodies, side effects, arguments, storage, or runtime
/// guards outside the emitted [`MachineGraph`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegalWalk<S: 'static, T: 'static> {
    /// State where the walk starts.
    pub start: S,
    /// Ordered transition choices made by the walk.
    pub steps: Vec<WalkStep<S, T>>,
    /// State reached after applying the selected targets in `steps`.
    pub end: S,
}

/// Builder for finite graph-metadata legal walks.
#[derive(Clone, Debug)]
pub struct Generator<S: 'static, T: 'static> {
    graph: &'static MachineGraph<S, T>,
    start: S,
    max_depth: usize,
    terminal_states: Vec<S>,
    include_empty: bool,
}

/// Starts metadata-only legal walk generation from `start`.
///
/// The generator observes only the active build's emitted [`MachineGraph`]. It
/// branches over every declared legal target for each transition site and stops
/// at `max_depth` or a caller-declared terminal state. If `start` is absent from
/// the graph, iteration is empty.
pub fn legal_walks_from<S, T>(graph: &'static MachineGraph<S, T>, start: S) -> Generator<S, T>
where
    S: Copy + Eq + 'static,
    T: Copy + Eq + 'static,
{
    Generator {
        graph,
        start,
        max_depth: graph.states.len().saturating_sub(1),
        terminal_states: Vec::new(),
        include_empty: true,
    }
}

impl<S, T> Generator<S, T>
where
    S: Copy + Eq + 'static,
    T: Copy + Eq + 'static,
{
    /// Limits generated walks to at most `depth` transition steps.
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Marks states where generation should stop even when outgoing transitions exist.
    pub fn terminal_states<I>(mut self, states: I) -> Self
    where
        I: IntoIterator<Item = S>,
    {
        self.terminal_states = states.into_iter().collect();
        self
    }

    /// Controls whether the zero-step walk from `start` to `start` is yielded.
    pub fn include_empty(mut self, include_empty: bool) -> Self {
        self.include_empty = include_empty;
        self
    }

    fn expand(
        &self,
        current: S,
        steps: &mut Vec<WalkStep<S, T>>,
        walks: &mut Vec<LegalWalk<S, T>>,
    ) {
        if self.include_empty || !steps.is_empty() {
            walks.push(LegalWalk {
                start: self.start,
                steps: steps.clone(),
                end: current,
            });
        }

        if steps.len() >= self.max_depth || self.terminal_states.contains(&current) {
            return;
        }

        for transition in self.graph.transitions_from(current) {
            for chosen_target in transition.to.iter().copied() {
                steps.push(WalkStep {
                    from: current,
                    transition: transition.id,
                    method_name: transition.method_name,
                    legal_targets: transition.to,
                    chosen_target,
                });
                self.expand(chosen_target, steps, walks);
                steps.pop();
            }
        }
    }
}

impl<S, T> IntoIterator for Generator<S, T>
where
    S: Copy + Eq + 'static,
    T: Copy + Eq + 'static,
{
    type Item = LegalWalk<S, T>;
    type IntoIter = IntoIter<LegalWalk<S, T>>;

    fn into_iter(self) -> Self::IntoIter {
        let mut walks = Vec::new();
        if self.graph.state(self.start).is_some() {
            self.expand(self.start, &mut Vec::new(), &mut walks);
        }
        walks.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::legal_walks_from;
    use crate::{
        MachineDescriptor, MachineGraph, StateDescriptor, TransitionDescriptor, TransitionInventory,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum StateId {
        Draft,
        Review,
        Published,
        Rejected,
        Archived,
        Missing,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum TransitionId {
        Submit,
        Decide,
        Archive,
        Reopen,
    }

    static REVIEW_TARGETS: [StateId; 1] = [StateId::Review];
    static DECIDE_TARGETS: [StateId; 2] = [StateId::Published, StateId::Rejected];
    static ARCHIVE_TARGETS: [StateId; 1] = [StateId::Archived];
    static REOPEN_TARGETS: [StateId; 1] = [StateId::Review];
    static STATES: [StateDescriptor<StateId>; 5] = [
        StateDescriptor {
            id: StateId::Draft,
            rust_name: "Draft",
            has_data: false,
        },
        StateDescriptor {
            id: StateId::Review,
            rust_name: "Review",
            has_data: false,
        },
        StateDescriptor {
            id: StateId::Published,
            rust_name: "Published",
            has_data: false,
        },
        StateDescriptor {
            id: StateId::Rejected,
            rust_name: "Rejected",
            has_data: false,
        },
        StateDescriptor {
            id: StateId::Archived,
            rust_name: "Archived",
            has_data: false,
        },
    ];
    static TRANSITIONS: [TransitionDescriptor<StateId, TransitionId>; 4] = [
        TransitionDescriptor {
            id: TransitionId::Submit,
            method_name: "submit",
            from: StateId::Draft,
            to: &REVIEW_TARGETS,
        },
        TransitionDescriptor {
            id: TransitionId::Decide,
            method_name: "decide",
            from: StateId::Review,
            to: &DECIDE_TARGETS,
        },
        TransitionDescriptor {
            id: TransitionId::Archive,
            method_name: "archive",
            from: StateId::Published,
            to: &ARCHIVE_TARGETS,
        },
        TransitionDescriptor {
            id: TransitionId::Reopen,
            method_name: "reopen",
            from: StateId::Rejected,
            to: &REOPEN_TARGETS,
        },
    ];
    static GRAPH: MachineGraph<StateId, TransitionId> = MachineGraph {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::DocumentMachine",
        },
        states: &STATES,
        transitions: TransitionInventory::new(|| &TRANSITIONS),
    };

    #[test]
    fn enumerates_branching_walk_prefixes_from_graph_metadata() {
        let walks = legal_walks_from(&GRAPH, StateId::Draft)
            .max_depth(2)
            .include_empty(false)
            .into_iter()
            .map(|walk| (walk.end, walk.steps.len()))
            .collect::<Vec<_>>();

        assert_eq!(
            walks,
            vec![
                (StateId::Review, 1),
                (StateId::Published, 2),
                (StateId::Rejected, 2)
            ]
        );
    }

    #[test]
    fn terminal_states_stop_expansion_before_outgoing_edges() {
        let walks = legal_walks_from(&GRAPH, StateId::Draft)
            .max_depth(3)
            .terminal_states([StateId::Review])
            .include_empty(false)
            .into_iter()
            .collect::<Vec<_>>();

        assert_eq!(walks.len(), 1);
        assert_eq!(walks[0].end, StateId::Review);
        assert_eq!(walks[0].steps[0].method_name, "submit");
    }

    #[test]
    fn max_depth_bounds_cycles() {
        let walks = legal_walks_from(&GRAPH, StateId::Rejected)
            .max_depth(3)
            .include_empty(false)
            .into_iter()
            .map(|walk| walk.end)
            .collect::<Vec<_>>();

        assert_eq!(
            walks,
            vec![
                StateId::Review,
                StateId::Published,
                StateId::Archived,
                StateId::Rejected,
                StateId::Review,
            ]
        );
    }

    #[test]
    fn absent_start_state_yields_no_walks() {
        let walks = legal_walks_from(&GRAPH, StateId::Missing)
            .max_depth(2)
            .into_iter()
            .collect::<Vec<_>>();

        assert!(walks.is_empty());
    }
}
