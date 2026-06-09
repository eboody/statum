use proc_macro2::Span;
use quote::ToTokens;
use syn::{AngleBracketedGenericArguments, GenericArgument, PathArguments, Type, TypePath};

#[derive(Clone, Copy)]
pub(crate) enum SupportedWrapper {
    Option,
    Result,
    Branch,
}

pub(crate) fn extract_impl_machine_and_state(
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

pub(crate) fn type_path(ty: &Type) -> Option<&TypePath> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    type_path.qself.is_none().then_some(type_path)
}

pub(crate) fn machine_segment_matching_target<'a>(
    candidate_path: &'a syn::Path,
    target_type: &Type,
) -> Option<&'a syn::PathSegment> {
    let target_path = &type_path(target_type)?.path;
    path_matches_target_machine(candidate_path, target_path)
        .then(|| candidate_path.segments.last())
        .flatten()
}

fn path_matches_target_machine(candidate: &syn::Path, target: &syn::Path) -> bool {
    paths_match_target_machine(candidate, target)
        || self_qualified_path_matches_target_machine(candidate, target)
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
    for (index, (candidate_segment, target_segment)) in candidate_segments
        .iter()
        .zip(target_segments.iter())
        .enumerate()
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

pub(crate) fn supported_wrapper(path: &syn::Path) -> Option<SupportedWrapper> {
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
        && path.segments.iter().zip(expected.iter()).enumerate().all(
            |(index, (segment, expected_ident))| {
                segment.ident == *expected_ident
                    && (index + 1 == expected.len()
                        || matches!(segment.arguments, PathArguments::None))
            },
        )
}

pub(crate) fn extract_machine_state_from_segment(
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

pub(crate) fn extract_first_generic_type_ref(args: &PathArguments) -> Option<&Type> {
    extract_generic_type_refs(args)?.into_iter().next()
}

pub(crate) fn extract_generic_type_refs(args: &PathArguments) -> Option<Vec<&Type>> {
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
