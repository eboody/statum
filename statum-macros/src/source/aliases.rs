use proc_macro2::Span;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use syn::visit_mut::VisitMut;
use syn::{GenericArgument, PathArguments, Type};

use super::{
    crate_root_for_file, current_module_path_opt, current_source_info, module_path_for_line,
    module_path_from_file_with_root, module_path_to_file, module_root_from_file,
    source_info_for_span, type_aliases_in_module,
};

#[derive(Clone, Debug)]
pub(crate) struct AliasResolutionContext {
    pub(crate) file_path: String,
    pub(crate) module_path: String,
    pub(crate) module_root: PathBuf,
    pub(crate) root_module_path: String,
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

fn type_path(ty: &Type) -> Option<&syn::TypePath> {
    match ty {
        Type::Path(type_path) if type_path.qself.is_none() => Some(type_path),
        _ => None,
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
    let (file_path, line_number) = source_info_for_span(span)?;
    let module_path = module_path_for_line(&file_path, line_number)?;
    Some(AliasResolutionContext {
        module_root: module_root_from_file(&file_path),
        root_module_path: source_observation_root_module(&file_path),
        file_path,
        module_path,
    })
}

pub(crate) fn candidate_alias_resolution_contexts(
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
    if let Some(crate_root) = crate_root_for_file(file_path) {
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
    let local_candidates = type_aliases_in_module(&context.file_path, &target_module, &alias_name);
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
    let candidates = type_aliases_in_module(&alias_file, &target_module, &alias_name);
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
            while segments
                .get(index)
                .is_some_and(|segment| segment == "super")
            {
                module = parent_module_path(&module)?;
                index += 1;
            }
            module
        }
        _ => return None,
    };

    for segment in &segments[index..alias_name_index] {
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

pub(crate) fn expand_source_type_alias(
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
