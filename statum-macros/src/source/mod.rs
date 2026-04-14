//! Source observation facade for file, module, and candidate facts used by the macros.

mod aliases;
mod analysis;
mod cache;
mod callsite;
mod module_path;
mod parser;
mod pathing;
mod query;
mod syntax;

pub(crate) use aliases::{
    AliasResolutionContext, candidate_alias_resolution_contexts, expand_source_type_alias,
};
pub(crate) use callsite::{
    current_module_path_opt, current_source_info, module_path_for_line, module_path_for_span,
    source_info_for_span,
};
pub(crate) use pathing::{
    module_path_from_file_with_root, module_path_to_file, module_root_from_file,
};
pub(crate) use query::{
    ItemCandidate, ItemKind, candidates_in_module, format_candidates, plain_item_line_in_module,
    same_named_candidates_elsewhere, type_aliases_in_module,
};
pub(crate) use syntax::{
    ItemTarget, ModulePath, SourceFingerprint, crate_root_for_file, current_crate_root,
    extract_derives, source_file_fingerprint,
};

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ObservedSourceContext {
    pub(crate) crate_root: Option<String>,
    pub(crate) file_path: String,
    pub(crate) line_number: usize,
    pub(crate) module_path: Option<String>,
    pub(crate) fingerprint: Option<SourceFingerprint>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ObservedMachineCandidates {
    pub(crate) same_module: Vec<ItemCandidate>,
    pub(crate) same_name_elsewhere: Vec<ItemCandidate>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ObservedStateCandidates {
    pub(crate) same_module: Vec<ItemCandidate>,
    pub(crate) same_name_elsewhere: Vec<ItemCandidate>,
}

#[allow(dead_code)]
pub(crate) fn observed_source_context_for_span(
    span: proc_macro2::Span,
) -> Option<ObservedSourceContext> {
    let (file_path, line_number) = source_info_for_span(span)?;
    let module_path = module_path_for_line(&file_path, line_number);

    Some(ObservedSourceContext {
        crate_root: crate_root_for_file(&file_path),
        fingerprint: source_file_fingerprint(&file_path),
        file_path,
        line_number,
        module_path,
    })
}

#[allow(dead_code)]
pub(crate) fn observed_current_source_context() -> Option<ObservedSourceContext> {
    let (file_path, line_number) = current_source_info()?;
    let module_path = module_path_for_line(&file_path, line_number);

    Some(ObservedSourceContext {
        crate_root: crate_root_for_file(&file_path),
        fingerprint: source_file_fingerprint(&file_path),
        file_path,
        line_number,
        module_path,
    })
}

#[allow(dead_code)]
pub(crate) fn observed_machine_candidates(
    file_path: &str,
    module_path: &str,
    machine_name: &str,
) -> ObservedMachineCandidates {
    ObservedMachineCandidates {
        same_module: candidates_in_module(
            file_path,
            module_path,
            ItemKind::Struct,
            Some("machine"),
        )
        .into_iter()
        .filter(|candidate| candidate.name == machine_name)
        .collect(),
        same_name_elsewhere: same_named_candidates_elsewhere(
            file_path,
            module_path,
            ItemKind::Struct,
            machine_name,
            Some("machine"),
        ),
    }
}

#[allow(dead_code)]
pub(crate) fn observed_state_candidates(
    file_path: &str,
    module_path: &str,
    state_enum_name: &str,
) -> ObservedStateCandidates {
    ObservedStateCandidates {
        same_module: candidates_in_module(file_path, module_path, ItemKind::Enum, Some("state"))
            .into_iter()
            .filter(|candidate| candidate.name == state_enum_name)
            .collect(),
        same_name_elsewhere: same_named_candidates_elsewhere(
            file_path,
            module_path,
            ItemKind::Enum,
            state_enum_name,
            Some("state"),
        ),
    }
}
