use crate::callsite::{current_module_path_opt, current_source_info};
use crate::pathing::{module_path_from_file_with_root, module_path_to_file, module_root_from_file};
use crate::query;
use proc_macro2::Span;
use quote::ToTokens;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use syn::visit_mut::VisitMut;
use syn::{AngleBracketedGenericArguments, GenericArgument, PathArguments, Type, TypePath};

pub(super) fn extract_impl_machine_and_state(
    target_type: &Type,
) -> Option<(String, Span, String, Span)> {
    let Type::Path(type_path) = target_type else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    extract_machine_state_from_segment(segment).map(|(_, state_name, state_span)| {
        (
            segment.ident.to_string(),
            segment.ident.span(),
            state_name,
            state_span,
        )
    })
}

#[derive(Clone, Debug)]
pub(super) struct AliasResolutionContext {
    file_path: String,
    module_path: String,
    module_root: PathBuf,
    root_module_path: String,
}

#[derive(Clone)]
struct ResolvedTypeAlias {
    item: syn::ItemType,
    context: AliasResolutionContext,
}

struct TypeAliasSubstituter<'a> {
    substitutions: &'a HashMap<String, Type>,
}

impl VisitMut for TypeAliasSubstituter<'_> {
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(type_path) = ty
            && type_path.qself.is_none()
            && type_path.path.leading_colon.is_none()
            && type_path.path.segments.len() == 1
        {
            let segment = &type_path.path.segments[0];
            if matches!(segment.arguments, PathArguments::None)
                && let Some(replacement) = self.substitutions.get(&segment.ident.to_string())
            {
                *ty = replacement.clone();
                return;
            }
        }

        syn::visit_mut::visit_type_mut(self, ty);
    }
}

fn current_alias_resolution_context() -> Option<AliasResolutionContext> {
    let (file_path, _) = current_source_info()?;
    let module_path = current_module_path_opt()?;
    Some(AliasResolutionContext {
        module_root: module_root_from_file(&file_path),
        root_module_path: source_observation_root_module(&file_path),
        file_path,
        module_path,
    })
}

fn current_alias_resolution_context_for_span(span: Span) -> Option<AliasResolutionContext> {
    let (file_path, line_number) = crate::callsite::source_info_for_span(span)?;
    let module_path = crate::callsite::module_path_for_line(&file_path, line_number)?;
    Some(AliasResolutionContext {
        module_root: module_root_from_file(&file_path),
        root_module_path: source_observation_root_module(&file_path),
        file_path,
        module_path,
    })
}

pub(super) fn candidate_alias_resolution_contexts(
    span: Option<Span>,
) -> Vec<AliasResolutionContext> {
    let mut contexts = Vec::new();

    if let Some(context) = current_alias_resolution_context() {
        contexts.push(context);
    }
    if let Some(span) = span
        && let Some(context) = current_alias_resolution_context_for_span(span)
        && !contexts.iter().any(|existing| {
            existing.file_path == context.file_path && existing.module_path == context.module_path
        })
    {
        contexts.push(context);
    }

    contexts
}

fn source_observation_root_module(file_path: &str) -> String {
    if let Some(crate_root) = crate::crate_root_for_file(file_path) {
        let src_root = PathBuf::from(crate_root).join("src");
        if PathBuf::from(file_path).starts_with(&src_root) {
            return "crate".to_owned();
        }
    }

    let module_root = module_root_from_file(file_path);
    module_path_from_file_with_root(file_path, &module_root)
}

fn resolve_type_alias(
    path: &syn::Path,
    context: &AliasResolutionContext,
) -> Option<ResolvedTypeAlias> {
    let alias_name = path.segments.last()?.ident.to_string();
    let target_module = alias_module_path(path, context)?;
    let local_candidates =
        query::type_aliases_in_module(&context.file_path, &target_module, &alias_name);
    if local_candidates.len() == 1 {
        let candidate = local_candidates.into_iter().next()?;
        return Some(ResolvedTypeAlias {
            item: candidate.item,
            context: AliasResolutionContext {
                file_path: context.file_path.clone(),
                module_path: target_module,
                module_root: context.module_root.clone(),
                root_module_path: context.root_module_path.clone(),
            },
        });
    }
    if local_candidates.len() > 1 {
        return None;
    }

    let alias_file = module_path_to_file(&target_module, &context.file_path, &context.module_root)?;
    let alias_file = alias_file.to_string_lossy().into_owned();
    let candidates = query::type_aliases_in_module(&alias_file, &target_module, &alias_name);
    if candidates.len() != 1 {
        return None;
    }

    let candidate = candidates.into_iter().next()?;
    Some(ResolvedTypeAlias {
        item: candidate.item,
        context: AliasResolutionContext {
            file_path: alias_file,
            module_path: target_module,
            module_root: context.module_root.clone(),
            root_module_path: context.root_module_path.clone(),
        },
    })
}

fn alias_module_path(path: &syn::Path, context: &AliasResolutionContext) -> Option<String> {
    if path.leading_colon.is_some() {
        return None;
    }

    let segments = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let alias_name_index = segments.len().checked_sub(1)?;
    if alias_name_index == 0 {
        return Some(context.module_path.to_owned());
    }

    let mut index = 0usize;
    let mut base = match segments.first()?.as_str() {
        "crate" => {
            index = 1;
            context.root_module_path.clone()
        }
        "self" => {
            index = 1;
            context.module_path.to_owned()
        }
        "super" => {
            let mut module = context.module_path.to_owned();
            while segments.get(index).is_some_and(|segment| segment == "super") {
                module = parent_module_path(&module)?;
                index += 1;
            }
            module
        }
        _ => return None,
    };

    for segment in segments[index..alias_name_index].iter() {
        base = child_module_path(&base, segment);
    }

    Some(base)
}

fn parent_module_path(module_path: &str) -> Option<String> {
    if module_path == "crate" {
        return None;
    }

    module_path
        .rsplit_once("::")
        .map(|(parent, _)| parent.to_owned())
        .or_else(|| Some("crate".to_owned()))
}

fn child_module_path(base: &str, child: &str) -> String {
    if base == "crate" {
        child.to_owned()
    } else {
        format!("{base}::{child}")
    }
}

fn instantiate_type_alias(item: &syn::ItemType, path: &syn::Path) -> Option<Type> {
    let segment = path.segments.last()?;
    let actual_type_args = match &segment.arguments {
        PathArguments::None => Vec::new(),
        PathArguments::AngleBracketed(args) => {
            if args
                .args
                .iter()
                .any(|arg| !matches!(arg, GenericArgument::Type(_)))
            {
                return None;
            }
            args.args
                .iter()
                .filter_map(|arg| match arg {
                    GenericArgument::Type(ty) => Some(ty.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        }
        PathArguments::Parenthesized(_) => return None,
    };

    let mut substitutions = HashMap::new();
    let mut actual_index = 0usize;
    for param in &item.generics.params {
        let syn::GenericParam::Type(type_param) = param else {
            return None;
        };

        let actual = if let Some(actual) = actual_type_args.get(actual_index) {
            actual_index += 1;
            actual.clone()
        } else if let Some(default) = &type_param.default {
            default.clone()
        } else {
            return None;
        };

        substitutions.insert(type_param.ident.to_string(), actual);
    }

    if actual_index != actual_type_args.len() {
        return None;
    }

    let mut expanded = (*item.ty).clone();
    TypeAliasSubstituter {
        substitutions: &substitutions,
    }
    .visit_type_mut(&mut expanded);
    Some(expanded)
}

pub(super) fn expand_source_type_alias(
    ty: &Type,
    context: Option<&AliasResolutionContext>,
    visited: &mut HashSet<String>,
) -> Option<(Type, AliasResolutionContext, String)> {
    let context = context?;
    let type_path = type_path(ty)?;
    let resolved = resolve_type_alias(&type_path.path, context)?;
    let visit_key = format!(
        "{}::{}::{}",
        resolved.context.file_path, resolved.context.module_path, resolved.item.ident
    );
    if !visited.insert(visit_key.clone()) {
        return None;
    }

    let expanded = instantiate_type_alias(&resolved.item, &type_path.path);
    if expanded.is_none() {
        visited.remove(&visit_key);
    }
    expanded.map(|expanded| (expanded, resolved.context, visit_key))
}

#[derive(Clone, Copy)]
pub(super) enum SupportedWrapper {
    Option,
    Result,
    Branch,
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_machine_and_state(ty: &Type, target_type: &Type) -> Option<(String, String)> {
    parse_primary_machine_and_state_in_context(ty, target_type, None)
}

pub(super) fn parse_machine_and_state_in_context(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
) -> Option<(String, String)> {
    parse_primary_machine_and_state_with_alias_policy(ty, target_type, context, true)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_primary_machine_and_state(
    ty: &Type,
    target_type: &Type,
) -> Option<(String, String)> {
    parse_primary_machine_and_state_with_alias_policy(ty, target_type, None, true)
}

fn parse_primary_machine_and_state_in_context(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
) -> Option<(String, String)> {
    parse_primary_machine_and_state_with_alias_policy(ty, target_type, context, true)
}

pub(super) fn parse_primary_machine_and_state_strict(
    ty: &Type,
    target_type: &Type,
) -> Option<(String, String)> {
    parse_primary_machine_and_state_with_alias_policy(ty, target_type, None, false)
}

fn parse_primary_machine_and_state_with_alias_policy(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
    allow_source_aliases: bool,
) -> Option<(String, String)> {
    let mut visited = HashSet::new();
    parse_primary_machine_and_state_inner(
        ty,
        target_type,
        context,
        allow_source_aliases,
        &mut visited,
    )
}

fn parse_primary_machine_and_state_inner(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
    allow_source_aliases: bool,
    visited: &mut HashSet<String>,
) -> Option<(String, String)> {
    let type_path = type_path(ty)?;
    let segment = type_path.path.segments.last()?;

    if machine_segment_matching_target(&type_path.path, target_type).is_some() {
        return extract_machine_state_from_segment(segment)
            .map(|(machine, state, _)| (machine, state));
    }

    if allow_source_aliases
        && let Some((expanded, alias_context, visit_key)) =
            expand_source_type_alias(ty, context, visited)
    {
        let result = parse_primary_machine_and_state_inner(
            &expanded,
            target_type,
            Some(&alias_context),
            allow_source_aliases,
            visited,
        );
        visited.remove(&visit_key);
        return result;
    }

    match supported_wrapper(&type_path.path)? {
        SupportedWrapper::Option => extract_first_generic_type_ref(&segment.arguments).and_then(
            |inner| {
                parse_primary_machine_and_state_inner(
                    inner,
                    target_type,
                    context,
                    allow_source_aliases,
                    visited,
                )
            },
        ),
        SupportedWrapper::Result | SupportedWrapper::Branch => {
            extract_first_generic_type_ref(&segment.arguments).and_then(|inner| {
                parse_primary_machine_and_state_inner(
                    inner,
                    target_type,
                    context,
                    allow_source_aliases,
                    visited,
                )
            })
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn collect_machine_and_states(ty: &Type, target_type: &Type) -> Vec<(String, String)> {
    collect_machine_and_states_with_alias_policy(ty, target_type, None, true)
}

pub(super) fn collect_machine_and_states_in_context(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
) -> Vec<(String, String)> {
    collect_machine_and_states_with_alias_policy(ty, target_type, context, true)
}

pub(super) fn collect_machine_and_states_strict(
    ty: &Type,
    target_type: &Type,
) -> Vec<(String, String)> {
    collect_machine_and_states_with_alias_policy(ty, target_type, None, false)
}

fn collect_machine_and_states_with_alias_policy(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
    allow_source_aliases: bool,
) -> Vec<(String, String)> {
    let mut targets = Vec::new();
    let mut visited = HashSet::new();
    collect_machine_targets_inner(
        ty,
        target_type,
        context,
        allow_source_aliases,
        &mut visited,
        &mut targets,
    );
    targets
}

fn collect_machine_targets_inner(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
    allow_source_aliases: bool,
    visited: &mut HashSet<String>,
    targets: &mut Vec<(String, String)>,
) {
    let Some(type_path) = type_path(ty) else {
        return;
    };
    let Some(segment) = type_path.path.segments.last() else {
        return;
    };

    if machine_segment_matching_target(&type_path.path, target_type).is_some() {
        if let Some((machine, state, _)) = extract_machine_state_from_segment(segment) {
            push_unique_target(targets, machine, state);
        }
        return;
    }

    if allow_source_aliases
        && let Some((expanded, alias_context, visit_key)) =
            expand_source_type_alias(ty, context, visited)
    {
        collect_machine_targets_inner(
            &expanded,
            target_type,
            Some(&alias_context),
            allow_source_aliases,
            visited,
            targets,
        );
        visited.remove(&visit_key);
        return;
    }

    match supported_wrapper(&type_path.path) {
        Some(SupportedWrapper::Option) => {
            if let Some(inner) = extract_first_generic_type_ref(&segment.arguments) {
                collect_machine_targets_inner(
                    inner,
                    target_type,
                    context,
                    allow_source_aliases,
                    visited,
                    targets,
                );
            }
        }
        Some(SupportedWrapper::Result | SupportedWrapper::Branch) => {
            if let Some(types) = extract_generic_type_refs(&segment.arguments) {
                for inner in types {
                    collect_machine_targets_inner(
                        inner,
                        target_type,
                        context,
                        allow_source_aliases,
                        visited,
                        targets,
                    );
                }
            }
        }
        None => {}
    }
}

fn push_unique_target(targets: &mut Vec<(String, String)>, machine: String, state: String) {
    if !targets
        .iter()
        .any(|(existing_machine, existing_state)| {
            existing_machine == &machine && existing_state == &state
        })
    {
        targets.push((machine, state));
    }
}

pub(super) fn type_path(ty: &Type) -> Option<&TypePath> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    type_path.qself.is_none().then_some(type_path)
}

pub(super) fn machine_segment_matching_target<'a>(
    candidate_path: &'a syn::Path,
    target_type: &Type,
) -> Option<&'a syn::PathSegment> {
    let target_path = &type_path(target_type)?.path;
    path_matches_target_machine(candidate_path, target_path)
        .then(|| candidate_path.segments.last())
        .flatten()
}

fn path_matches_target_machine(candidate: &syn::Path, target: &syn::Path) -> bool {
    paths_match_target_machine(candidate, target) || self_qualified_path_matches_target_machine(candidate, target)
}

fn paths_match_target_machine(candidate: &syn::Path, target: &syn::Path) -> bool {
    if candidate.leading_colon.is_some() != target.leading_colon.is_some() {
        return false;
    }

    path_segments_match_target_machine(candidate.segments.iter(), target.segments.iter())
}

fn self_qualified_path_matches_target_machine(candidate: &syn::Path, target: &syn::Path) -> bool {
    if candidate.leading_colon.is_some() || target.leading_colon.is_some() {
        return false;
    }

    let mut candidate_segments = candidate.segments.iter();
    let Some(self_segment) = candidate_segments.next() else {
        return false;
    };
    if self_segment.ident != "self" || !matches!(self_segment.arguments, PathArguments::None) {
        return false;
    }

    path_segments_match_target_machine(candidate_segments, target.segments.iter())
}

fn path_segments_match_target_machine<'a>(
    candidate_segments: impl Iterator<Item = &'a syn::PathSegment>,
    target_segments: impl Iterator<Item = &'a syn::PathSegment>,
) -> bool {
    let candidate_segments = candidate_segments.collect::<Vec<_>>();
    let target_segments = target_segments.collect::<Vec<_>>();
    if candidate_segments.len() != target_segments.len() {
        return false;
    }

    let last_index = candidate_segments.len().saturating_sub(1);
    for (index, (candidate_segment, target_segment)) in
        candidate_segments.iter().zip(target_segments.iter()).enumerate()
    {
        if candidate_segment.ident != target_segment.ident {
            return false;
        }

        let arguments_match = if index == last_index {
            machine_generic_arguments_match(&candidate_segment.arguments, &target_segment.arguments)
        } else {
            path_arguments_equal(&candidate_segment.arguments, &target_segment.arguments)
        };

        if !arguments_match {
            return false;
        }
    }

    true
}

fn machine_generic_arguments_match(candidate: &PathArguments, target: &PathArguments) -> bool {
    let PathArguments::AngleBracketed(candidate_args) = candidate else {
        return false;
    };
    let PathArguments::AngleBracketed(target_args) = target else {
        return false;
    };
    if candidate_args.args.len() != target_args.args.len() || candidate_args.args.is_empty() {
        return false;
    }

    matches!(candidate_args.args.first(), Some(GenericArgument::Type(_)))
        && matches!(target_args.args.first(), Some(GenericArgument::Type(_)))
        && candidate_args
            .args
            .iter()
            .skip(1)
            .map(argument_tokens)
            .eq(target_args.args.iter().skip(1).map(argument_tokens))
}

fn path_arguments_equal(left: &PathArguments, right: &PathArguments) -> bool {
    argument_tokens(left) == argument_tokens(right)
}

fn argument_tokens<T: ToTokens>(tokens: &T) -> String {
    tokens.to_token_stream().to_string()
}

pub(super) fn supported_wrapper(path: &syn::Path) -> Option<SupportedWrapper> {
    if matches_absolute_type_path(path, &["core", "option", "Option"])
        || matches_absolute_type_path(path, &["std", "option", "Option"])
    {
        return Some(SupportedWrapper::Option);
    }

    if matches_absolute_type_path(path, &["core", "result", "Result"])
        || matches_absolute_type_path(path, &["std", "result", "Result"])
    {
        return Some(SupportedWrapper::Result);
    }

    if matches_absolute_type_path(path, &["statum", "Branch"])
        || matches_absolute_type_path(path, &["statum_core", "Branch"])
    {
        return Some(SupportedWrapper::Branch);
    }

    None
}

fn matches_absolute_type_path(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_some()
        && path.segments.len() == expected.len()
        && path
            .segments
            .iter()
            .zip(expected.iter())
            .enumerate()
            .all(|(index, (segment, expected_ident))| {
                segment.ident == *expected_ident
                    && (index + 1 == expected.len()
                        || matches!(segment.arguments, PathArguments::None))
            })
}

pub(super) fn extract_machine_state_from_segment(
    segment: &syn::PathSegment,
) -> Option<(String, String, Span)> {
    extract_machine_generic(&segment.arguments, &segment.ident.to_string())
}

fn extract_machine_generic(
    args: &PathArguments,
    machine_name: &str,
) -> Option<(String, String, Span)> {
    let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
        args: generic_args, ..
    }) = args
    else {
        return None;
    };
    let first_generic = generic_args.iter().find_map(|arg| match arg {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })?;
    let (state_name, state_span) = extract_state_marker(first_generic)?;
    Some((machine_name.to_string(), state_name, state_span))
}

fn extract_state_marker(ty: &Type) -> Option<(String, Span)> {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return None;
    };
    if path.leading_colon.is_some() || path.segments.len() != 1 {
        return None;
    }

    let state_segment = path.segments.last()?;
    if !matches!(state_segment.arguments, PathArguments::None) {
        return None;
    }

    Some((state_segment.ident.to_string(), state_segment.ident.span()))
}

fn extract_first_generic_type_ref(args: &PathArguments) -> Option<&Type> {
    extract_generic_type_refs(args)?.into_iter().next()
}

pub(super) fn extract_generic_type_refs(args: &PathArguments) -> Option<Vec<&Type>> {
    let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
        args: generic_args, ..
    }) = args
    else {
        return None;
    };

    let types = generic_args
        .iter()
        .filter_map(|arg| match arg {
            GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .collect::<Vec<_>>();
    if types.is_empty() {
        return None;
    }

    Some(types)
}

#[cfg(test)]
mod tests {
    use super::{
        AliasResolutionContext, collect_machine_and_states, collect_machine_and_states_in_context,
        extract_impl_machine_and_state, module_root_from_file, parse_machine_and_state,
        parse_machine_and_state_in_context, parse_primary_machine_and_state,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use syn::Type;

    fn parse_type(source: &str) -> Type {
        syn::parse_str(source).expect("valid type")
    }

    fn write_temp_rust_file(contents: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let crate_dir = std::env::temp_dir().join(format!("statum_transition_alias_{nanos}"));
        let src_dir = crate_dir.join("src");
        fs::create_dir_all(&src_dir).expect("create temp crate");
        let path = src_dir.join("lib.rs");
        fs::write(&path, contents).expect("write temp file");
        path
    }

    #[test]
    fn primary_parser_preserves_existing_result_behavior() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("::core::result::Result<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(
            parse_primary_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            parse_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
    }

    #[test]
    fn target_collector_reads_both_result_branches() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("::core::result::Result<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![
                ("Machine".to_owned(), "Accepted".to_owned()),
                ("Machine".to_owned(), "Rejected".to_owned()),
            ]
        );
    }

    #[test]
    fn primary_parser_reads_first_branch_target() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("::statum::Branch<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(
            parse_primary_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            parse_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
    }

    #[test]
    fn target_collector_reads_both_branch_targets() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("::statum::Branch<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![
                ("Machine".to_owned(), "Accepted".to_owned()),
                ("Machine".to_owned(), "Rejected".to_owned()),
            ]
        );
    }

    #[test]
    fn target_collector_reads_nested_wrappers() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type(
            "::core::option::Option<::core::result::Result<Machine<Accepted>, ::statum::Branch<Machine<Rejected>, Error>>>",
        );

        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![
                ("Machine".to_owned(), "Accepted".to_owned()),
                ("Machine".to_owned(), "Rejected".to_owned()),
            ]
        );
    }

    #[test]
    fn target_collector_ignores_non_machine_payloads_and_dedups() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type(
            "::core::result::Result<::core::option::Option<Machine<Accepted>>, ::core::result::Result<Machine<Accepted>, Error>>",
        );

        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![("Machine".to_owned(), "Accepted".to_owned())]
        );
    }

    #[test]
    fn parser_rejects_bare_wrappers() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("Result<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(parse_machine_and_state(&ty, &target), None);
        assert!(collect_machine_and_states(&ty, &target).is_empty());
    }

    #[test]
    fn parser_rejects_same_leaf_machine_in_other_module() {
        let target = parse_type("FlowMachine<Draft>");
        let ty = parse_type("other::FlowMachine<Done>");

        assert_eq!(parse_machine_and_state(&ty, &target), None);
        assert!(collect_machine_and_states(&ty, &target).is_empty());
    }

    #[test]
    fn parser_accepts_std_wrapper_paths() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type(
            "::std::option::Option<::std::result::Result<Machine<Accepted>, Error>>",
        );

        assert_eq!(
            parse_primary_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![("Machine".to_owned(), "Accepted".to_owned())]
        );
    }

    #[test]
    fn parser_accepts_self_qualified_machine_paths() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("::core::option::Option<self::Machine<Accepted>>");

        assert_eq!(
            parse_primary_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![("Machine".to_owned(), "Accepted".to_owned())]
        );
    }

    #[test]
    fn impl_target_rejects_qualified_state_paths() {
        let ty = parse_type("Machine<crate::Draft>");
        assert!(extract_impl_machine_and_state(&ty).is_none());
    }

    #[test]
    fn parser_resolves_crate_root_aliases_from_submodules() {
        let path = write_temp_rust_file(
            r#"
pub type Result<T> = ::core::result::Result<T, ()>;
pub type Flow<State> = Machine<State>;

mod auth {
    pub fn marker() {}
}
"#,
        );
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("crate::Result<crate::Flow<Accepted>>");
        let context = AliasResolutionContext {
            module_root: module_root_from_file(path.to_str().expect("path")),
            root_module_path: "crate".into(),
            file_path: path.to_string_lossy().into_owned(),
            module_path: "auth".into(),
        };

        assert_eq!(
            parse_machine_and_state_in_context(&ty, &target, Some(&context)),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            collect_machine_and_states_in_context(&ty, &target, Some(&context)),
            vec![("Machine".to_owned(), "Accepted".to_owned())]
        );

        let _ = fs::remove_dir_all(path.parent().expect("src").parent().expect("crate"));
    }

    #[test]
    fn parser_resolves_crate_root_aliases_in_real_fixture_file() {
        let path = format!(
            "{}/tests/ui/valid_transition_crate_aliases.rs",
            env!("CARGO_MANIFEST_DIR")
        );
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("crate::Result<crate::Flow<Accepted>>");
        let context = AliasResolutionContext {
            module_root: module_root_from_file(&path),
            root_module_path: "valid_transition_crate_aliases".into(),
            file_path: path,
            module_path: "valid_transition_crate_aliases::auth".into(),
        };

        assert_eq!(
            parse_machine_and_state_in_context(&ty, &target, Some(&context)),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            collect_machine_and_states_in_context(&ty, &target, Some(&context)),
            vec![("Machine".to_owned(), "Accepted".to_owned())]
        );
    }
}
