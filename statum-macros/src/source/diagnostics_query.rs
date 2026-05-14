use super::{
    ItemCandidate, ItemKind, candidates_in_module, current_source_info, plain_item_line_in_module,
    same_named_candidates_elsewhere,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AnnotatedItemKind {
    Machine,
    State,
}

impl AnnotatedItemKind {
    fn item_kind(self) -> ItemKind {
        match self {
            Self::Machine => ItemKind::Struct,
            Self::State => ItemKind::Enum,
        }
    }

    fn required_attr(self) -> &'static str {
        match self {
            Self::Machine => "machine",
            Self::State => "state",
        }
    }
}

/// Source-backed lookup contract for annotated Statum items in a module.
///
/// This stays at the source-observation layer: it returns possible source matches and plain
/// same-named items, but does not decide whether a candidate is semantically resolvable.
pub(crate) struct SourceModuleQuery<'a> {
    anchor: SourceModuleQueryAnchor<'a>,
}

enum SourceModuleQueryAnchor<'a> {
    CurrentModule {
        module_path: &'a str,
    },
    ExplicitFile {
        file_path: &'a str,
        module_path: &'a str,
    },
}

impl<'a> SourceModuleQuery<'a> {
    pub(crate) fn current(module_path: &'a str) -> Self {
        Self {
            anchor: SourceModuleQueryAnchor::CurrentModule { module_path },
        }
    }

    pub(crate) fn anchored_at(file_path: Option<&'a str>, module_path: &'a str) -> Self {
        match file_path {
            Some(file_path) => Self {
                anchor: SourceModuleQueryAnchor::ExplicitFile {
                    file_path,
                    module_path,
                },
            },
            None => Self::current(module_path),
        }
    }

    pub(crate) fn machine_candidates(&self) -> Vec<ItemCandidate> {
        self.candidates(AnnotatedItemKind::Machine)
    }

    pub(crate) fn state_candidates(&self) -> Vec<ItemCandidate> {
        self.candidates(AnnotatedItemKind::State)
    }

    pub(crate) fn same_named_machine_candidates_elsewhere(
        &self,
        machine_name: &str,
    ) -> Option<Vec<ItemCandidate>> {
        self.same_named_candidates_elsewhere(AnnotatedItemKind::Machine, machine_name)
    }

    pub(crate) fn same_named_state_candidates_elsewhere(
        &self,
        state_name: &str,
    ) -> Option<Vec<ItemCandidate>> {
        self.same_named_candidates_elsewhere(AnnotatedItemKind::State, state_name)
    }

    pub(crate) fn plain_machine_struct_line(&self, machine_name: &str) -> Option<usize> {
        self.plain_item_line(AnnotatedItemKind::Machine, machine_name)
    }

    pub(crate) fn plain_state_enum_line(&self, state_name: &str) -> Option<usize> {
        self.plain_item_line(AnnotatedItemKind::State, state_name)
    }

    fn candidates(&self, kind: AnnotatedItemKind) -> Vec<ItemCandidate> {
        let Some(file_path) = self.resolve_file_path() else {
            return Vec::new();
        };
        candidates_in_module(
            &file_path,
            self.module_path(),
            kind.item_kind(),
            Some(kind.required_attr()),
        )
    }

    fn same_named_candidates_elsewhere(
        &self,
        kind: AnnotatedItemKind,
        item_name: &str,
    ) -> Option<Vec<ItemCandidate>> {
        let file_path = self.resolve_file_path()?;
        let candidates = same_named_candidates_elsewhere(
            &file_path,
            self.module_path(),
            kind.item_kind(),
            item_name,
            Some(kind.required_attr()),
        );
        (!candidates.is_empty()).then_some(candidates)
    }

    fn plain_item_line(&self, kind: AnnotatedItemKind, item_name: &str) -> Option<usize> {
        let file_path = self.resolve_file_path()?;
        plain_item_line_in_module(
            &file_path,
            self.module_path(),
            kind.item_kind(),
            item_name,
            Some(kind.required_attr()),
        )
    }

    fn resolve_file_path(&self) -> Option<String> {
        match self.anchor {
            SourceModuleQueryAnchor::CurrentModule { .. } => {
                current_source_info().map(|(path, _)| path)
            }
            SourceModuleQueryAnchor::ExplicitFile { file_path, .. } => Some(file_path.to_owned()),
        }
    }

    fn module_path(&self) -> &'a str {
        match self.anchor {
            SourceModuleQueryAnchor::CurrentModule { module_path }
            | SourceModuleQueryAnchor::ExplicitFile { module_path, .. } => module_path,
        }
    }
}
