use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{
    AngleBracketedGenericArguments, GenericArgument, PathArguments, Type, TypeArray, TypeGroup,
    TypeParen, TypePath, TypeReference, TypeSlice, TypeTuple,
};

#[derive(Clone)]
pub(crate) enum RelationTargetCandidate {
    DirectMachine {
        machine_path: Vec<String>,
        state_name: String,
    },
    DeclaredReferenceType {
        ty: Box<Type>,
    },
}

pub(crate) fn collect_relation_targets(
    ty: &Type,
    source_module_path: &str,
) -> Vec<RelationTargetCandidate> {
    let mut targets = Vec::new();
    collect_relation_targets_inner(ty, source_module_path, &mut targets);
    targets
}

pub(crate) fn parse_machine_reference_target(
    ty: &Type,
    source_module_path: &str,
) -> Result<(Vec<String>, String), TokenStream> {
    machine_link_candidate(ty, source_module_path)
        .map(|candidate| (candidate.machine_path, candidate.state_name))
        .ok_or_else(|| {
            syn::Error::new_spanned(
                ty,
                "Error: `#[machine_ref(...)]` expects one explicit machine target like `crate::task::Machine<crate::task::Running>`.\nFix: point it at one concrete Statum machine state using an explicit `crate::`, `self::`, `super::`, or absolute path.",
            )
            .to_compile_error()
        })
}

pub(crate) fn leading_type_ident(ty: &Type) -> Option<&syn::Ident> {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return None;
    };

    let segment = path.segments.first()?;
    matches!(segment.arguments, PathArguments::None).then_some(&segment.ident)
}

fn collect_relation_targets_inner(
    ty: &Type,
    source_module_path: &str,
    targets: &mut Vec<RelationTargetCandidate>,
) {
    match ty {
        Type::Path(type_path) => collect_path_targets(type_path, source_module_path, targets),
        Type::Reference(TypeReference { elem, .. }) => {
            collect_relation_targets_inner(elem, source_module_path, targets);
        }
        Type::Tuple(TypeTuple { elems, .. }) => {
            for elem in elems {
                collect_relation_targets_inner(elem, source_module_path, targets);
            }
        }
        Type::Array(TypeArray { elem, .. })
        | Type::Slice(TypeSlice { elem, .. })
        | Type::Group(TypeGroup { elem, .. })
        | Type::Paren(TypeParen { elem, .. }) => {
            collect_relation_targets_inner(elem, source_module_path, targets)
        }
        _ => {}
    }
}

fn collect_path_targets(
    type_path: &TypePath,
    source_module_path: &str,
    targets: &mut Vec<RelationTargetCandidate>,
) {
    if type_path.qself.is_some() {
        return;
    }

    if let Some(wrapper) = supported_wrapper(&type_path.path) {
        collect_wrapper_targets(wrapper, &type_path.path, source_module_path, targets);
        return;
    }

    if let Some(candidate) = machine_link_candidate(&Type::Path(type_path.clone()), source_module_path)
    {
        push_unique_target(
            targets,
            RelationTargetCandidate::DirectMachine {
                machine_path: candidate.machine_path,
                state_name: candidate.state_name,
            },
        );
        return;
    }

    if declared_reference_candidate(type_path) {
        push_unique_target(
            targets,
            RelationTargetCandidate::DeclaredReferenceType {
                ty: Box::new(Type::Path(type_path.clone())),
            },
        );
    }
}

fn collect_wrapper_targets(
    wrapper: SupportedWrapper,
    path: &syn::Path,
    source_module_path: &str,
    targets: &mut Vec<RelationTargetCandidate>,
) {
    let Some(segment) = path.segments.last() else {
        return;
    };

    match wrapper {
        SupportedWrapper::Unary => {
            if let Some(inner) = extract_first_generic_type_ref(&segment.arguments) {
                collect_relation_targets_inner(inner, source_module_path, targets);
            }
        }
        SupportedWrapper::Binary => {
            if let Some(types) = extract_generic_type_refs(&segment.arguments) {
                for inner in types {
                    collect_relation_targets_inner(inner, source_module_path, targets);
                }
            }
        }
    }
}

fn push_unique_target(targets: &mut Vec<RelationTargetCandidate>, candidate: RelationTargetCandidate) {
    if targets.iter().any(|existing| same_target(existing, &candidate)) {
        return;
    }

    targets.push(candidate);
}

fn same_target(left: &RelationTargetCandidate, right: &RelationTargetCandidate) -> bool {
    match (left, right) {
        (
            RelationTargetCandidate::DirectMachine {
                machine_path: left_machine,
                state_name: left_state,
            },
            RelationTargetCandidate::DirectMachine {
                machine_path: right_machine,
                state_name: right_state,
            },
        ) => left_machine == right_machine && left_state == right_state,
        (
            RelationTargetCandidate::DeclaredReferenceType { ty: left_ty },
            RelationTargetCandidate::DeclaredReferenceType { ty: right_ty },
        ) => left_ty.to_token_stream().to_string() == right_ty.to_token_stream().to_string(),
        _ => false,
    }
}

struct MachineLinkCandidate {
    machine_path: Vec<String>,
    state_name: String,
}

fn machine_link_candidate(ty: &Type, source_module_path: &str) -> Option<MachineLinkCandidate> {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return None;
    };
    let segment = path.segments.last()?;
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let target_state_path = arguments.args.iter().find_map(|argument| match argument {
        GenericArgument::Type(ty) => exact_state_path(ty, source_module_path),
        _ => None,
    })?;
    let machine_path = exact_machine_path(path, source_module_path)?;
    if machine_path.len() < 2 || target_state_path.len() < 2 {
        return None;
    }
    let machine_module = &machine_path[..machine_path.len().saturating_sub(1)];
    let state_module = &target_state_path[..target_state_path.len().saturating_sub(1)];
    if machine_module != state_module {
        return None;
    }
    let state_name = target_state_path.last()?.clone();

    Some(MachineLinkCandidate {
        machine_path,
        state_name,
    })
}

fn declared_reference_candidate(type_path: &TypePath) -> bool {
    type_path
        .path
        .segments
        .iter()
        .all(|segment| matches!(segment.arguments, PathArguments::None))
}

enum SupportedWrapper {
    Unary,
    Binary,
}

fn supported_wrapper(path: &syn::Path) -> Option<SupportedWrapper> {
    if matches_absolute_type_path(path, &["core", "option", "Option"])
        || matches_absolute_type_path(path, &["std", "option", "Option"])
    {
        return Some(SupportedWrapper::Unary);
    }

    if matches_absolute_type_path(path, &["alloc", "vec", "Vec"])
        || matches_absolute_type_path(path, &["std", "vec", "Vec"])
    {
        return Some(SupportedWrapper::Unary);
    }

    if matches_absolute_type_path(path, &["alloc", "boxed", "Box"])
        || matches_absolute_type_path(path, &["std", "boxed", "Box"])
    {
        return Some(SupportedWrapper::Unary);
    }

    if matches_absolute_type_path(path, &["alloc", "rc", "Rc"])
        || matches_absolute_type_path(path, &["std", "rc", "Rc"])
    {
        return Some(SupportedWrapper::Unary);
    }

    if matches_absolute_type_path(path, &["alloc", "sync", "Arc"])
        || matches_absolute_type_path(path, &["std", "sync", "Arc"])
    {
        return Some(SupportedWrapper::Unary);
    }

    if matches_absolute_type_path(path, &["core", "result", "Result"])
        || matches_absolute_type_path(path, &["std", "result", "Result"])
    {
        return Some(SupportedWrapper::Binary);
    }

    None
}

fn exact_machine_path(path: &syn::Path, source_module_path: &str) -> Option<Vec<String>> {
    exact_path_segments(path, source_module_path, true)
}

fn exact_state_path(ty: &Type, source_module_path: &str) -> Option<Vec<String>> {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return None;
    };

    exact_path_segments(path, source_module_path, false)
}

fn exact_path_segments(
    path: &syn::Path,
    source_module_path: &str,
    allow_final_args: bool,
) -> Option<Vec<String>> {
    let raw_segments = path
        .segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            let is_final = index + 1 == path.segments.len();
            if (!is_final || !allow_final_args)
                && !matches!(segment.arguments, PathArguments::None)
            {
                return None;
            }
            Some(segment.ident.to_string())
        })
        .collect::<Option<Vec<_>>>()?;
    if raw_segments.is_empty() {
        return None;
    }

    if path.leading_colon.is_some() {
        return Some(raw_segments);
    }

    let module_segments = split_module_path(source_module_path);
    let mut resolved = Vec::new();
    let mut index = 0;
    match raw_segments.first()?.as_str() {
        "crate" => {
            resolved.push(module_segments.first()?.clone());
            index = 1;
        }
        "self" => {
            resolved.extend(module_segments);
            index = 1;
        }
        "super" => {
            resolved.extend(module_segments);
            while matches!(raw_segments.get(index).map(String::as_str), Some("super")) {
                if resolved.len() <= 1 {
                    return None;
                }
                resolved.pop();
                index += 1;
            }
        }
        _ => return None,
    }

    resolved.extend(raw_segments.into_iter().skip(index));
    Some(resolved)
}

fn split_module_path(module_path: &str) -> Vec<String> {
    module_path
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect()
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

fn extract_first_generic_type_ref(arguments: &PathArguments) -> Option<&Type> {
    let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) = arguments
    else {
        return None;
    };

    args.iter().find_map(|argument| match argument {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

fn extract_generic_type_refs(arguments: &PathArguments) -> Option<Vec<&Type>> {
    let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) = arguments
    else {
        return None;
    };

    let mut types = Vec::new();
    for argument in args {
        if let GenericArgument::Type(ty) = argument {
            types.push(ty);
        }
    }
    (!types.is_empty()).then_some(types)
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::Type;

    use super::{RelationTargetCandidate, collect_relation_targets, parse_machine_reference_target};

    fn parse_type(input: &str) -> Type {
        syn::parse_str(input).expect("type")
    }

    #[test]
    fn collects_direct_machine_targets_through_supported_wrappers() {
        let ty = parse_type(
            "::core::option::Option<::alloc::vec::Vec<crate::task::Machine<crate::task::Running>>>",
        );
        let targets = collect_relation_targets(&ty, "workspace::workflow");

        assert_eq!(targets.len(), 1);
        match &targets[0] {
            RelationTargetCandidate::DirectMachine {
                machine_path,
                state_name,
            } => {
                assert_eq!(
                    machine_path,
                    &vec![
                        "workspace".to_string(),
                        "task".to_string(),
                        "Machine".to_string()
                    ]
                );
                assert_eq!(state_name, "Running");
            }
            other => panic!("unexpected target: {:?}", core::mem::discriminant(other)),
        }
    }

    #[test]
    fn treats_named_types_as_declared_reference_candidates() {
        let ty = parse_type("::core::option::Option<TaskId>");
        let targets = collect_relation_targets(&ty, "workspace::workflow");

        assert_eq!(targets.len(), 1);
        match &targets[0] {
            RelationTargetCandidate::DeclaredReferenceType { ty } => {
                assert_eq!(quote! { #ty }.to_string(), "TaskId");
            }
            other => panic!("unexpected target: {:?}", core::mem::discriminant(other)),
        }
    }

    #[test]
    fn skips_generic_named_types_as_declared_reference_candidates() {
        let ty = parse_type("::core::option::Option<TaskId<Uuid>>");
        let targets = collect_relation_targets(&ty, "workspace::workflow");

        assert!(!targets.iter().any(|target| matches!(
            target,
            RelationTargetCandidate::DeclaredReferenceType { .. }
        )));
    }

    #[test]
    fn parses_machine_ref_target_shape() {
        let ty = parse_type("crate::task::Machine<crate::task::Running>");
        let (machine_path, state_name) =
            parse_machine_reference_target(&ty, "workspace::workflow")
                .expect("machine ref target");

        assert_eq!(
            machine_path,
            vec![
                "workspace".to_string(),
                "task".to_string(),
                "Machine".to_string()
            ]
        );
        assert_eq!(state_name, "Running");
    }

    #[test]
    fn rejects_non_machine_ref_target_shape() {
        let ty = parse_type("TaskId");
        assert!(parse_machine_reference_target(&ty, "workspace::workflow").is_err());
    }

    #[test]
    fn resolves_self_qualified_machine_ref_targets() {
        let ty = parse_type("self::task::Machine<self::task::Running>");
        let (machine_path, state_name) =
            parse_machine_reference_target(&ty, "workspace::workflow").expect("machine ref target");

        assert_eq!(
            machine_path,
            vec![
                "workspace".to_string(),
                "workflow".to_string(),
                "task".to_string(),
                "Machine".to_string()
            ]
        );
        assert_eq!(state_name, "Running");
    }

    #[test]
    fn rejects_super_qualified_machine_ref_targets_above_crate_root() {
        let ty = parse_type("super::task::Machine<super::task::Running>");
        assert!(parse_machine_reference_target(&ty, "workspace").is_err());
    }

    #[test]
    fn rejects_noncanonical_same_name_wrappers() {
        let ty = parse_type("foo::Option<crate::task::Machine<crate::task::Running>>");
        let targets = collect_relation_targets(&ty, "workspace::workflow");

        assert!(targets.is_empty());
    }

    #[test]
    fn rejects_unanchored_direct_machine_paths() {
        let ty = parse_type("task::Machine<task::Running>");
        let targets = collect_relation_targets(&ty, "workspace::workflow");

        assert!(targets.is_empty());
    }
}
