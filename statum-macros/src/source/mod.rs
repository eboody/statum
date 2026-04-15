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
