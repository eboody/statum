use crate::{MachineGraph, TransitionDescriptor};
use core::fmt::Debug;

/// Starts an assertion that a source state has a named transition to a target.
pub fn assert_transition<S, T>(graph: &'static MachineGraph<S, T>) -> TransitionAssertion<S, T>
where
    S: Copy + Eq + Debug + 'static,
    T: Copy + Eq + Debug + 'static,
{
    TransitionAssertion {
        graph,
        from: None,
        method: None,
    }
}

/// Starts an assertion that a source state does not have a named transition.
pub fn assert_no_transition<S, T>(graph: &'static MachineGraph<S, T>) -> NoTransitionAssertion<S, T>
where
    S: Copy + Eq + Debug + 'static,
    T: Copy + Eq + Debug + 'static,
{
    NoTransitionAssertion { graph, from: None }
}

/// Starts an assertion over the complete target set for a transition site.
pub fn assert_targets<S, T>(graph: &'static MachineGraph<S, T>) -> TargetAssertion<S, T>
where
    S: Copy + Eq + Debug + 'static,
    T: Copy + Eq + Debug + 'static,
{
    TargetAssertion {
        graph,
        from: None,
        method: None,
    }
}

/// Starts an assertion that graph metadata contains a directed path between states.
///
/// This observes only the emitted [`MachineGraph`] metadata. A state has a
/// zero-length path to itself when that state is present in the graph, including
/// terminal states with no outgoing transitions.
pub fn assert_path<S, T>(graph: &'static MachineGraph<S, T>) -> PathAssertion<S, T>
where
    S: Copy + Eq + Debug + 'static,
    T: Copy + Eq + Debug + 'static,
{
    PathAssertion { graph, from: None }
}

/// Starts an assertion that graph metadata contains no directed path between states.
///
/// This observes only the emitted [`MachineGraph`] metadata. Missing or
/// disconnected states are treated deterministically as having no path.
pub fn assert_no_path<S, T>(graph: &'static MachineGraph<S, T>) -> NoPathAssertion<S, T>
where
    S: Copy + Eq + Debug + 'static,
    T: Copy + Eq + Debug + 'static,
{
    NoPathAssertion { graph, from: None }
}

/// Builder for `assert_transition(graph).from(source).method(name).to(target)`.
#[derive(Clone, Copy, Debug)]
pub struct TransitionAssertion<S: 'static, T: 'static> {
    graph: &'static MachineGraph<S, T>,
    from: Option<S>,
    method: Option<&'static str>,
}

impl<S, T> TransitionAssertion<S, T>
where
    S: Copy + Eq + Debug + 'static,
    T: Copy + Eq + Debug + 'static,
{
    /// Selects the source state for the assertion.
    pub fn from(mut self, state: S) -> Self {
        self.from = Some(state);
        self
    }

    /// Selects the transition method name for the assertion.
    pub fn method(mut self, method: &'static str) -> Self {
        self.method = Some(method);
        self
    }

    /// Asserts that the transition exists and includes `target` in its legal targets.
    pub fn to(self, target: S) {
        let transition = self.require_transition();
        assert!(
            transition.to.contains(&target),
            "expected transition {}::{:?}.{} to allow target {:?}, but legal targets were {:?}",
            self.graph.machine.rust_type_path,
            transition.from,
            transition.method_name,
            target,
            transition.to
        );
    }

    fn require_transition(&self) -> &TransitionDescriptor<S, T> {
        let from = self.from.unwrap_or_else(|| {
            panic!(
                "missing source state for transition assertion on {}",
                self.graph.machine.rust_type_path
            )
        });
        let method = self.method.unwrap_or_else(|| {
            panic!(
                "missing method name for transition assertion on {}::{:?}",
                self.graph.machine.rust_type_path, from
            )
        });

        self.graph
            .transition_from_method(from, method)
            .unwrap_or_else(|| {
                panic!(
                    "expected transition {}::{:?}.{} to exist, but no transition with that source and method was emitted",
                    self.graph.machine.rust_type_path, from, method
                )
            })
    }
}

/// Builder for `assert_no_transition(graph).from(source).method(name)`.
#[derive(Clone, Copy, Debug)]
pub struct NoTransitionAssertion<S: 'static, T: 'static> {
    graph: &'static MachineGraph<S, T>,
    from: Option<S>,
}

impl<S, T> NoTransitionAssertion<S, T>
where
    S: Copy + Eq + Debug + 'static,
    T: Copy + Eq + Debug + 'static,
{
    /// Selects the source state for the assertion.
    pub fn from(mut self, state: S) -> Self {
        self.from = Some(state);
        self
    }

    /// Asserts that no transition with this source and method exists.
    pub fn method(self, method: &str) {
        let from = self.from.unwrap_or_else(|| {
            panic!(
                "missing source state for no-transition assertion on {}",
                self.graph.machine.rust_type_path
            )
        });

        if let Some(transition) = self.graph.transition_from_method(from, method) {
            panic!(
                "expected transition {}::{:?}.{} to be absent, but GRAPH contains {:?} --{}--> {:?}",
                self.graph.machine.rust_type_path,
                from,
                method,
                transition.from,
                transition.method_name,
                transition.to
            );
        }
    }
}

/// Builder for `assert_targets(graph).from(source).method(name).exactly(targets)`.
#[derive(Clone, Copy, Debug)]
pub struct TargetAssertion<S: 'static, T: 'static> {
    graph: &'static MachineGraph<S, T>,
    from: Option<S>,
    method: Option<&'static str>,
}

impl<S, T> TargetAssertion<S, T>
where
    S: Copy + Eq + Debug + 'static,
    T: Copy + Eq + Debug + 'static,
{
    /// Selects the source state for the assertion.
    pub fn from(mut self, state: S) -> Self {
        self.from = Some(state);
        self
    }

    /// Selects the transition method name for the assertion.
    pub fn method(mut self, method: &'static str) -> Self {
        self.method = Some(method);
        self
    }

    /// Asserts that the emitted legal target slice exactly matches `expected` in order.
    pub fn exactly<I>(self, expected: I)
    where
        I: IntoIterator<Item = S>,
    {
        let assertion = TransitionAssertion {
            graph: self.graph,
            from: self.from,
            method: self.method,
        };
        let transition = assertion.require_transition();
        let expected = expected.into_iter().collect::<Vec<_>>();
        assert_eq!(
            transition.to, expected.as_slice(),
            "expected transition {}::{:?}.{} to have exact legal targets {:?}, but legal targets were {:?}",
            self.graph.machine.rust_type_path, transition.from, transition.method_name, expected, transition.to
        );
    }
}

/// Builder for `assert_path(graph).from(source).to(target)`.
#[derive(Clone, Copy, Debug)]
pub struct PathAssertion<S: 'static, T: 'static> {
    graph: &'static MachineGraph<S, T>,
    from: Option<S>,
}

impl<S, T> PathAssertion<S, T>
where
    S: Copy + Eq + Debug + 'static,
    T: Copy + Eq + Debug + 'static,
{
    /// Selects the source state for the path assertion.
    pub fn from(mut self, state: S) -> Self {
        self.from = Some(state);
        self
    }

    /// Asserts that graph metadata contains a directed path from the selected source to `target`.
    pub fn to(self, target: S) {
        let from = self.require_from("path");
        assert!(
            graph_has_path(self.graph, from, target),
            "expected graph metadata for {} to contain a path from {:?} to {:?}, but no directed path was emitted",
            self.graph.machine.rust_type_path,
            from,
            target
        );
    }

    fn require_from(&self, assertion_kind: &str) -> S {
        self.from.unwrap_or_else(|| {
            panic!(
                "missing source state for {assertion_kind} assertion on {}",
                self.graph.machine.rust_type_path
            )
        })
    }
}

/// Builder for `assert_no_path(graph).from(source).to(target)`.
#[derive(Clone, Copy, Debug)]
pub struct NoPathAssertion<S: 'static, T: 'static> {
    graph: &'static MachineGraph<S, T>,
    from: Option<S>,
}

impl<S, T> NoPathAssertion<S, T>
where
    S: Copy + Eq + Debug + 'static,
    T: Copy + Eq + Debug + 'static,
{
    /// Selects the source state for the no-path assertion.
    pub fn from(mut self, state: S) -> Self {
        self.from = Some(state);
        self
    }

    /// Asserts that graph metadata contains no directed path from the selected source to `target`.
    pub fn to(self, target: S) {
        let from = self.from.unwrap_or_else(|| {
            panic!(
                "missing source state for no-path assertion on {}",
                self.graph.machine.rust_type_path
            )
        });
        assert!(
            !graph_has_path(self.graph, from, target),
            "expected graph metadata for {} to contain no path from {:?} to {:?}, but a directed path was emitted",
            self.graph.machine.rust_type_path,
            from,
            target
        );
    }
}

fn graph_has_path<S, T>(graph: &MachineGraph<S, T>, from: S, target: S) -> bool
where
    S: Copy + Eq + 'static,
    T: Copy + Eq + 'static,
{
    if graph.state(from).is_none() || graph.state(target).is_none() {
        return false;
    }

    let mut visited = Vec::new();
    let mut pending = vec![from];

    while let Some(state) = pending.pop() {
        if visited.contains(&state) {
            continue;
        }
        if state == target {
            return true;
        }
        visited.push(state);

        for transition in graph.transitions_from(state) {
            for candidate in transition.to.iter().copied().rev() {
                if !visited.contains(&candidate) {
                    pending.push(candidate);
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::{assert_no_path, assert_path};
    use crate::{
        MachineDescriptor, MachineGraph, StateDescriptor, TransitionDescriptor, TransitionInventory,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum StateId {
        Draft,
        Review,
        Published,
        Archived,
        Missing,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum TransitionId {
        Submit,
        Publish,
    }

    static REVIEW_TARGETS: [StateId; 1] = [StateId::Review];
    static PUBLISHED_TARGETS: [StateId; 1] = [StateId::Published];
    static STATES: [StateDescriptor<StateId>; 4] = [
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
            id: StateId::Archived,
            rust_name: "Archived",
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
            to: &PUBLISHED_TARGETS,
        },
    ];
    static GRAPH: MachineGraph<StateId, TransitionId> = MachineGraph {
        machine: MachineDescriptor {
            module_path: "workflow",
            rust_type_path: "workflow::Machine",
        },
        states: &STATES,
        transitions: TransitionInventory::new(|| &TRANSITIONS),
    };

    #[test]
    fn asserts_transitive_path_exists_through_graph_metadata() {
        assert_path(&GRAPH)
            .from(StateId::Draft)
            .to(StateId::Published);
    }

    #[test]
    fn asserts_terminal_and_disconnected_paths_deterministically() {
        assert_path(&GRAPH)
            .from(StateId::Published)
            .to(StateId::Published);
        assert_no_path(&GRAPH)
            .from(StateId::Published)
            .to(StateId::Draft);
        assert_no_path(&GRAPH)
            .from(StateId::Archived)
            .to(StateId::Published);
        assert_no_path(&GRAPH)
            .from(StateId::Missing)
            .to(StateId::Published);
        assert_no_path(&GRAPH)
            .from(StateId::Draft)
            .to(StateId::Missing);
    }
}
