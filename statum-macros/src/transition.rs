use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::{
    punctuated::Punctuated, token::Comma, AngleBracketedGenericArguments, Attribute, Block, Expr,
    FnArg, GenericArgument, Ident, ImplItem, ImplItemFn, ItemImpl, LitStr, Pat, Path,
    PathArguments, PathSegment, ReturnType, Type, TypePath,
};

use crate::machine::to_shouty_snake_identifier;
use crate::relation::{RelationTargetCandidate, collect_relation_targets, leading_type_ident};
use crate::{
    PresentationAttr, parse_doc_attrs, parse_present_attrs, strip_present_attrs, to_snake_case,
};

/// Stores all metadata for a single transition method in an `impl` block
#[allow(unused)]
pub struct TransitionFn {
    pub name: Ident,
    pub attrs: Vec<syn::Attribute>,
    pub docs: Option<String>,
    pub presentation: Option<PresentationAttr>,
    pub has_receiver: bool,
    pub return_type: Option<Type>,
    pub return_type_span: Option<Span>,
    pub machine_name: String,
    pub source_state: String,
    pub parameters: Vec<TransitionParam>,
    pub generics: Vec<Ident>,
    pub sig_generics: syn::Generics,
    pub internals: Block,
    pub is_async: bool,
    pub vis: syn::Visibility,
    pub span: proc_macro2::Span,
}

#[allow(unused)]
pub struct TransitionParam {
    pub name: Option<String>,
    pub binding_ident: Ident,
    pub ty: Type,
    pub via_routes: Vec<ViaRoute>,
    pub span: Span,
}

#[allow(unused)]
pub struct ViaRoute {
    pub display_path: String,
    pub via_module_path: String,
    pub route_name: String,
    pub route_type: Type,
    pub route_id: u64,
}

impl TransitionFn {
    pub fn return_state(&self, target_type: &Type) -> Result<String, TokenStream> {
        let Some(return_type) = self.return_type.as_ref() else {
            return Err(invalid_return_type_error(self, "missing return type"));
        };
        let Some((_, return_state)) = parse_machine_and_state(return_type, target_type) else {
            return Err(invalid_return_type_error(
                self,
                "expected the impl target machine path directly, or the same path wrapped in a canonical `::core::option::Option`, `::core::result::Result`, or `::statum::Branch`",
            ));
        };

        Ok(return_state)
    }

    pub fn return_states(&self, target_type: &Type) -> Result<Vec<String>, TokenStream> {
        let Some(return_type) = self.return_type.as_ref() else {
            return Err(invalid_return_type_error(self, "missing return type"));
        };
        let return_states = collect_machine_and_states(return_type, target_type)
            .into_iter()
            .map(|(_, state)| state)
            .collect::<Vec<_>>();
        if return_states.is_empty() {
            return Err(invalid_return_type_error(
                self,
                "expected the impl target machine path directly, or the same path wrapped in a canonical `::core::option::Option`, `::core::result::Result`, or `::statum::Branch`",
            ));
        }

        Ok(return_states)
    }
}

/// Represents the entire `impl` block of our `transition` macro
pub struct TransitionImpl {
    /// The concrete type being implemented (e.g. `Machine<Draft>`)
    pub target_type: Type,
    /// The machine type name extracted from `target_type` (e.g. `Machine`)
    pub machine_name: String,
    /// The source state extracted from `target_type` (e.g. `Draft`)
    pub source_state: String,
    /// `module_path!()` for the module that owns this transition impl site.
    pub module_path: String,
    pub generic_params: Vec<String>,
    pub attrs: Vec<syn::Attribute>,
    /// All transition methods extracted from the `impl`
    pub functions: Vec<TransitionFn>,
}

pub fn parse_transition_impl(
    item_impl: &ItemImpl,
    module_path: &str,
) -> Result<TransitionImpl, TokenStream> {
    let target_type = *item_impl.self_ty.clone();
    let Some((machine_name, _, source_state, _)) = extract_impl_machine_and_state(&target_type) else {
        let message = LitStr::new(
            "Invalid #[transition] target type. Expected an impl target like `Machine<State>`.",
            target_type.span(),
        );
        return Err(quote::quote_spanned! { target_type.span() =>
            compile_error!(#message);
        });
    };

    let mut functions = Vec::new();
    for item in &item_impl.items {
        if let ImplItem::Fn(method) = item {
            functions.push(parse_transition_fn(
                method,
                &machine_name,
                &source_state,
                module_path,
            )?);
        }
    }
    let generic_params = item_impl
        .generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Type(type_param) => Some(type_param.ident.to_string()),
            _ => None,
        })
        .collect();

    Ok(TransitionImpl {
        target_type,
        machine_name,
        source_state,
        module_path: module_path.to_owned(),
        generic_params,
        attrs: item_impl.attrs.clone(),
        functions,
    })
}

fn extract_impl_machine_and_state(target_type: &Type) -> Option<(String, Span, String, Span)> {
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

pub fn parse_transition_fn(
    method: &ImplItemFn,
    machine_name: &str,
    source_state: &str,
    source_module_path: &str,
) -> Result<TransitionFn, TokenStream> {
    let has_receiver = matches!(method.sig.inputs.first(), Some(FnArg::Receiver(_)));

    let return_type = match &method.sig.output {
        ReturnType::Type(_, ty) => Some(*ty.clone()),
        ReturnType::Default => None,
    };
    let return_type_span = match &method.sig.output {
        ReturnType::Type(_, ty) => Some(ty.span()),
        ReturnType::Default => None,
    };

    let generics = method
        .sig
        .generics
        .params
        .iter()
        .filter_map(|param| {
            if let syn::GenericParam::Type(type_param) = param {
                Some(type_param.ident.clone())
            } else {
                None
            }
        })
        .collect();
    let parameters = method
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Receiver(_) => None,
            FnArg::Typed(pat_ty) => Some(parse_transition_param(
                pat_ty,
                source_module_path,
                method.sig.ident.span(),
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let is_async = method.sig.asyncness.is_some();

    Ok(TransitionFn {
        name: method.sig.ident.clone(),
        attrs: method.attrs.clone(),
        docs: parse_doc_attrs(&method.attrs).map_err(|err| err.to_compile_error())?,
        presentation: parse_present_attrs(&method.attrs).map_err(|err| err.to_compile_error())?,
        has_receiver,
        return_type,
        return_type_span,
        machine_name: machine_name.to_owned(),
        source_state: source_state.to_owned(),
        parameters,
        generics,
        sig_generics: method.sig.generics.clone(),
        internals: method.block.clone(),
        is_async,
        vis: method.vis.to_owned(),
        span: method.span(),
    })
}

fn parse_transition_param(
    pat_ty: &syn::PatType,
    source_module_path: &str,
    fallback_span: Span,
) -> Result<TransitionParam, TokenStream> {
    let name = match pat_ty.pat.as_ref() {
        Pat::Ident(ident) => Some(ident.ident.to_string()),
        _ => None,
    };
    let binding_ident = match pat_ty.pat.as_ref() {
        Pat::Ident(ident) => ident.ident.clone(),
        _ => format_ident!(
            "__statum_arg_{}",
            stable_hash(&pat_ty.pat.to_token_stream().to_string())
        ),
    };
    let via_routes = parse_via_routes(&pat_ty.attrs, source_module_path, fallback_span)?;

    Ok(TransitionParam {
        name,
        binding_ident,
        ty: (*pat_ty.ty).clone(),
        via_routes,
        span: pat_ty.span(),
    })
}

fn parse_via_routes(
    attrs: &[Attribute],
    source_module_path: &str,
    fallback_span: Span,
) -> Result<Vec<ViaRoute>, TokenStream> {
    let via_attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("via"))
        .collect::<Vec<_>>();
    if via_attrs.is_empty() {
        return Ok(Vec::new());
    }
    if via_attrs.len() > 1 {
        return Err(compile_error_at(
            via_attrs[1].span(),
            "Error: transition parameters support at most one `#[via(...)]` attribute.",
        ));
    }
    let via_attr = via_attrs[0];
    let paths = via_attr
        .parse_args_with(Punctuated::<Path, Comma>::parse_terminated)
        .map_err(|err| err.to_compile_error())?;
    if paths.is_empty() {
        return Err(compile_error_at(
            via_attr.span(),
            "Error: `#[via(...)]` must list at least one attested route path.",
        ));
    }

    let mut routes = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        let route = parse_via_route(&path, source_module_path, fallback_span)?;
        if !seen.insert(route.display_path.clone()) {
            return Err(compile_error_at(
                path.span(),
                "Error: duplicate attested route path in `#[via(...)]`.",
            ));
        }
        routes.push(route);
    }

    Ok(routes)
}

fn parse_via_route(
    path: &Path,
    source_module_path: &str,
    fallback_span: Span,
) -> Result<ViaRoute, TokenStream> {
    if path
        .segments
        .iter()
        .any(|segment| !matches!(segment.arguments, PathArguments::None))
    {
        return Err(compile_error_at(
            path.span(),
            "Error: `#[via(...)]` only accepts simple route paths such as `crate::payment::via::Capture`.",
        ));
    }
    let Some(resolved_segments) = explicit_path_segments(path, source_module_path) else {
        return Err(compile_error_at(
            path.span(),
            "Error: `#[via(...)]` expects an explicit `crate::`, `self::`, `super::`, or absolute route path such as `crate::payment::via::Capture`.",
        ));
    };
    if resolved_segments.len() < 3
        || resolved_segments
            .get(resolved_segments.len().saturating_sub(2))
            .is_none_or(|segment| segment != "via")
    {
        return Err(compile_error_at(
            path.span(),
            "Error: `#[via(...)]` paths must end in `::via::<Route>`.",
        ));
    }

    let route_name = resolved_segments
        .last()
        .cloned()
        .expect("validated route name");
    let route_id = stable_hash(&route_name);
    let via_module_path = resolved_segments[..resolved_segments.len() - 1].join("::");
    let route_type = via_route_type(path, route_id, fallback_span)?;

    Ok(ViaRoute {
        display_path: path.to_token_stream().to_string(),
        via_module_path,
        route_name,
        route_type,
        route_id,
    })
}

fn via_route_type(path: &Path, route_id: u64, fallback_span: Span) -> Result<Type, TokenStream> {
    let mut route_path = path.clone();
    let Some(last_segment) = route_path.segments.last_mut() else {
        return Err(compile_error_at(
            fallback_span,
            "Error: invalid `#[via(...)]` route path.",
        ));
    };
    let route_expr = Expr::Verbatim(quote! { #route_id });
    let route_segment: PathSegment = syn::parse_quote! { Route<{ #route_expr }> };
    *last_segment = route_segment;
    syn::parse2(quote! { #route_path }).map_err(|err| err.to_compile_error())
}

fn explicit_path_segments(path: &Path, source_module_path: &str) -> Option<Vec<String>> {
    let raw_segments = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
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
pub fn validate_transition_functions(
    tr_impl: &TransitionImpl,
) -> Option<TokenStream> {
    if tr_impl.functions.is_empty() {
        let message = format!(
            "Error: #[transition] impl for `{}<{}>` must contain at least one method returning `{}` or the same machine path wrapped in a canonical `::core::option::Option<{}>`, `::core::result::Result<{}, E>`, or `::statum::Branch<{}, {}>`.",
            tr_impl.machine_name,
            tr_impl.source_state,
            machine_return_signature(&tr_impl.machine_name),
            machine_return_signature(&tr_impl.machine_name),
            machine_return_signature(&tr_impl.machine_name),
            machine_return_signature(&tr_impl.machine_name),
            machine_return_signature(&tr_impl.machine_name),
        );
        return Some(compile_error_at(tr_impl.target_type.span(), &message));
    }

    for func in &tr_impl.functions {
        if !func.has_receiver {
            let message = format!(
                "Error: `#[transition]` method `{}<{}>::{}` must take `self` or `mut self` as its receiver.",
                tr_impl.machine_name,
                tr_impl.source_state,
                func.name,
            );
            return Some(compile_error_at(func.span, &message));
        }

        let return_state = match func.return_state(&tr_impl.target_type) {
            Ok(state) => state,
            Err(err) => return Some(err),
        };
        let return_states = match func.return_states(&tr_impl.target_type) {
            Ok(states) => states,
            Err(err) => return Some(err),
        };
        drop(return_state);
        drop(return_states);

        let via_param_count = func
            .parameters
            .iter()
            .filter(|param| !param.via_routes.is_empty())
            .count();
        if via_param_count > 1 {
            let message = format!(
                "Error: `#[transition]` method `{}<{}>::{}` supports at most one `#[via(...)]` parameter in v1.",
                tr_impl.machine_name,
                tr_impl.source_state,
                func.name,
            );
            return Some(compile_error_at(func.span, &message));
        }
    }

    None
}

pub fn generate_transition_impl(
    input: &ItemImpl,
    tr_impl: &TransitionImpl,
) -> TokenStream {
    let target_type = &tr_impl.target_type;
    let (impl_generics, _, where_clause) = input.generics.split_for_impl();
    let transition_support_module_path = match replace_machine_leaf_ident(
        target_type,
        format_ident!("__statum_{}_transition", to_snake_case(&tr_impl.machine_name)),
    ) {
        Ok(path) => path,
        Err(err) => return err,
    };
    let machine_module_path = match replace_machine_leaf_ident(
        target_type,
        format_ident!("{}", to_snake_case(&tr_impl.machine_name)),
    ) {
        Ok(path) => path,
        Err(err) => return err,
    };
    let machine_target_resolver_macro_path = match replace_machine_leaf_ident(
        target_type,
        format_ident!(
            "__statum_resolve_{}_transition_target",
            to_snake_case(&tr_impl.machine_name)
        ),
    ) {
        Ok(path) => path,
        Err(err) => return err,
    };

    let mut emitted_states = HashSet::new();
    let mut unique_return_state_idents = Vec::new();
    for function in &tr_impl.functions {
        let return_states = match function.return_states(target_type) {
            Ok(states) => states,
            Err(err) => return err,
        };

        for return_state in return_states {
            if !emitted_states.insert(return_state.clone()) {
                continue;
            }
            unique_return_state_idents.push(format_ident!("{}", return_state));
        }
    }
    let transition_binding_callback_ident = format_ident!(
        "__statum_emit_{}_{}_transition_binding_{}",
        to_snake_case(&tr_impl.machine_name),
        to_snake_case(&tr_impl.source_state),
        transition_impl_unique_suffix(tr_impl)
    );
    let transition_support_bindings = quote! {
        #[doc(hidden)]
        macro_rules! #transition_binding_callback_ident {
            (target = $target_variant:ident, has_data = true) => {
                #[allow(dead_code)]
                impl #impl_generics #transition_support_module_path::EdgeTo<$target_variant>
                    for #target_type #where_clause
                {}

                #[allow(dead_code)]
                impl #impl_generics #transition_support_module_path::TransitionWithBinding<
                    <$target_variant as statum::StateMarker>::Data,
                > for #target_type #where_clause
                {
                    type NextState = $target_variant;
                }
            };
            (target = $target_variant:ident, has_data = false) => {
                #[allow(dead_code)]
                impl #impl_generics #transition_support_module_path::EdgeTo<$target_variant>
                    for #target_type #where_clause
                {}
            };
        }

        #( #machine_target_resolver_macro_path!(#transition_binding_callback_ident, #unique_return_state_idents); )*
    };
    let (relation_machine_module_path_string, relation_machine_rust_type_path_string) =
        match relation_source_machine_descriptor(tr_impl) {
            Ok(value) => value,
            Err(err) => return err,
        };
    let relation_machine_module_path =
        LitStr::new(&relation_machine_module_path_string, Span::call_site());
    let relation_machine_rust_type_path =
        LitStr::new(&relation_machine_rust_type_path_string, Span::call_site());
    let transition_registrations = tr_impl.functions.iter().enumerate().map(|(idx, function)| {
        let return_states = match function.return_states(target_type) {
            Ok(states) => states,
            Err(err) => return err,
        };
        let unique_suffix = transition_site_unique_suffix(tr_impl, function, idx);
        let token_ident = format_ident!("__STATUM_TRANSITION_TOKEN_{}", unique_suffix);
        let targets_ident = format_ident!("__STATUM_TRANSITION_TARGETS_{}", unique_suffix);
        let registration_ident = format_ident!("__STATUM_TRANSITION_SITE_{}", unique_suffix);
        let linked_registration_ident =
            format_ident!("__STATUM_LINKED_TRANSITION_SITE_{}", unique_suffix);
        let id_const_ident = format_ident!(
            "{}",
            to_shouty_snake_identifier(&function.name.to_string())
        );
        let method_name = LitStr::new(&function.name.to_string(), function.name.span());
        let source_state_ident = format_ident!("{}", tr_impl.source_state);
        let source_state_name = LitStr::new(&tr_impl.source_state, function.name.span());
        let linked_label = optional_lit_str_tokens(
            function
                .presentation
                .as_ref()
                .and_then(|value| value.label.as_deref()),
            function.name.span(),
        );
        let linked_description = optional_lit_str_tokens(
            function
                .presentation
                .as_ref()
                .and_then(|value| value.description.as_deref()),
            function.name.span(),
        );
        let linked_docs =
            optional_lit_str_tokens(function.docs.as_deref(), function.name.span());
        let target_state_idents = return_states.iter().map(|state| {
            let state_ident = format_ident!("{}", state);
            quote! { #machine_module_path::StateId::#state_ident }
        });
        let target_state_names = return_states.iter().map(|state| {
            let state = LitStr::new(state, function.name.span());
            quote! { #state }
        });
        let target_state_count = return_states.len();
        let cfg_attrs = propagated_cfg_attrs(&tr_impl.attrs, &function.attrs);
        let relation_registrations = transition_param_relation_registrations(
            tr_impl,
            function,
            idx,
            &relation_machine_module_path,
            &relation_machine_rust_type_path,
            &cfg_attrs,
        );
        let via_route_registration = attested_route_registration(
            tr_impl,
            function,
            idx,
            &relation_machine_module_path,
            &relation_machine_rust_type_path,
            &cfg_attrs,
        );

        quote! {
            #(#cfg_attrs)*
            static #targets_ident: [#machine_module_path::StateId; #target_state_count] = [
                #(#target_state_idents),*
            ];

            #(#cfg_attrs)*
            static #token_ident: statum::__private::TransitionToken =
                statum::__private::TransitionToken::new();

            #(#cfg_attrs)*
            #[statum::__private::linkme::distributed_slice(#machine_module_path::__STATUM_TRANSITIONS)]
            #[linkme(crate = statum::__private::linkme)]
            static #registration_ident:
                statum::TransitionDescriptor<#machine_module_path::StateId, #machine_module_path::TransitionId> =
                statum::TransitionDescriptor {
                    id: #machine_module_path::TransitionId::from_token(&#token_ident),
                    method_name: #method_name,
                    from: #machine_module_path::StateId::#source_state_ident,
                    to: &#targets_ident,
                };

            #(#cfg_attrs)*
            #[statum::__private::linkme::distributed_slice(#machine_module_path::__STATUM_LINKED_TRANSITIONS)]
            #[linkme(crate = statum::__private::linkme)]
            static #linked_registration_ident: statum::__private::LinkedTransitionDescriptor =
                statum::__private::LinkedTransitionDescriptor {
                    method_name: #method_name,
                    label: #linked_label,
                    description: #linked_description,
                    docs: #linked_docs,
                    from: #source_state_name,
                    to: &[#(#target_state_names),*],
                };

            #(#cfg_attrs)*
            impl #impl_generics #target_type #where_clause {
                pub const #id_const_ident: #machine_module_path::TransitionId =
                    #machine_module_path::TransitionId::from_token(&#token_ident);
            }

            #via_route_registration
            #(#relation_registrations)*
        }
    });
    let transition_presentation_registrations = tr_impl.functions.iter().enumerate().filter_map(|(idx, function)| {
        let presentation = function.presentation.as_ref()?;
        let unique_suffix = transition_site_unique_suffix(tr_impl, function, idx);
        let token_ident = format_ident!("__STATUM_TRANSITION_TOKEN_{}", unique_suffix);
        let registration_ident =
            format_ident!("__STATUM_TRANSITION_PRESENTATION_{}", unique_suffix);
        let label = optional_lit_str_tokens(presentation.label.as_deref(), function.name.span());
        let description =
            optional_lit_str_tokens(presentation.description.as_deref(), function.name.span());
        let metadata = match transition_presentation_metadata_tokens(presentation) {
            Ok(tokens) => tokens,
            Err(err) => return Some(err),
        };
        let cfg_attrs = propagated_cfg_attrs(&tr_impl.attrs, &function.attrs);

        Some(quote! {
            #(#cfg_attrs)*
            #[statum::__private::linkme::distributed_slice(#machine_module_path::__STATUM_TRANSITION_PRESENTATIONS)]
            #[linkme(crate = statum::__private::linkme)]
            static #registration_ident:
                statum::__private::TransitionPresentation<
                    #machine_module_path::TransitionId,
                    #machine_module_path::__StatumTransitionPresentationMetadata,
                > =
                statum::__private::TransitionPresentation {
                    id: #machine_module_path::TransitionId::from_token(&#token_ident),
                    label: #label,
                    description: #description,
                    metadata: #metadata,
                };
        })
    });
    let attested_companions = generate_attested_companion_methods(
        &input.generics,
        target_type,
        where_clause,
        tr_impl,
        &machine_module_path,
    );
    let via_binders = generate_via_binders(&input.generics, target_type, where_clause, tr_impl);
    let sanitized_input = strip_transition_impl_attrs(input);

    quote! {
        #transition_support_bindings
        #(#transition_registrations)*
        #(#transition_presentation_registrations)*
        #sanitized_input
        #attested_companions
        #via_binders
    }
}

fn strip_transition_impl_attrs(input: &ItemImpl) -> ItemImpl {
    let mut sanitized = input.clone();
    sanitized.attrs = strip_present_attrs(&sanitized.attrs);
    for item in &mut sanitized.items {
        if let ImplItem::Fn(method) = item {
            method.attrs = strip_present_attrs(&method.attrs);
            for arg in &mut method.sig.inputs {
                if let FnArg::Typed(pat_ty) = arg {
                    pat_ty.attrs = strip_via_attrs(&pat_ty.attrs);
                }
            }
        }
    }
    sanitized
}

fn strip_via_attrs(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("via"))
        .cloned()
        .collect()
}

fn attested_route_registration(
    tr_impl: &TransitionImpl,
    function: &TransitionFn,
    function_index: usize,
    relation_machine_module_path: &LitStr,
    relation_machine_rust_type_path: &LitStr,
    cfg_attrs: &[syn::Attribute],
) -> TokenStream {
    let Some(target_state) = direct_return_state(function, &tr_impl.target_type) else {
        return quote! {};
    };

    let unique_suffix = transition_site_unique_suffix(tr_impl, function, function_index);
    let registration_ident = format_ident!("__STATUM_LINKED_VIA_ROUTE_{}", unique_suffix);
    let route_name = route_marker_name(&function.name.to_string());
    let route_name_lit = LitStr::new(&route_name, function.name.span());
    let route_id = stable_hash(&route_name);
    let method_name = LitStr::new(&function.name.to_string(), function.name.span());
    let source_state = LitStr::new(&tr_impl.source_state, function.name.span());
    let target_state = LitStr::new(&target_state, function.name.span());
    let via_module_path = LitStr::new(
        &format!(
            "{}::{}::via",
            relation_machine_module_path.value(),
            to_snake_case(&tr_impl.machine_name)
        ),
        function.name.span(),
    );

    quote! {
        #(#cfg_attrs)*
        #[doc(hidden)]
        #[statum::__private::linkme::distributed_slice(statum::__private::__STATUM_LINKED_VIA_ROUTES)]
        #[linkme(crate = statum::__private::linkme)]
        static #registration_ident: statum::__private::LinkedViaRouteDescriptor =
            statum::__private::LinkedViaRouteDescriptor {
                machine: statum::MachineDescriptor {
                    module_path: #relation_machine_module_path,
                    rust_type_path: #relation_machine_rust_type_path,
                },
                via_module_path: #via_module_path,
                route_name: #route_name_lit,
                route_id: #route_id,
                transition: #method_name,
                source_state: #source_state,
                target_state: #target_state,
            };
    }
}

fn generate_attested_companion_methods(
    input_generics: &syn::Generics,
    target_type: &Type,
    where_clause: Option<&syn::WhereClause>,
    tr_impl: &TransitionImpl,
    machine_module_path: &Path,
) -> TokenStream {
    let methods = tr_impl
        .functions
        .iter()
        .filter_map(|function| {
            let Some(_target_state) = direct_return_state(function, target_type) else {
                return None;
            };
            let cfg_attrs = propagated_cfg_attrs(&tr_impl.attrs, &function.attrs);
            let return_type = function.return_type.as_ref()?;
            let route_name = route_marker_name(&function.name.to_string());
            let route_id = stable_hash(&route_name);
            let companion_ident = format_ident!("{}_and_attest", function.name);
            let asyncness = function.is_async.then_some(quote! { async });
            let await_tokens = function.is_async.then_some(quote! { .await });
            let params = function.parameters.iter().map(|param| {
                let binding_ident = &param.binding_ident;
                let ty = &param.ty;
                quote! { #binding_ident: #ty }
            });
            let call_args = function
                .parameters
                .iter()
                .map(|param| {
                    let binding_ident = &param.binding_ident;
                    quote! { #binding_ident }
                })
                .collect::<Vec<_>>();
            let vis = &function.vis;
            let sig_generics = &function.sig_generics;
            let method_ident = &function.name;
            let route_type: Type =
                syn::parse_quote! { #machine_module_path::via::Route<{ #route_id }> };
            let route_return_ty = quote! { statum::__private::Attested<#return_type, #route_type> };
            Some(quote! {
                #(#cfg_attrs)*
                #vis #asyncness fn #companion_ident #sig_generics (
                    self,
                    #(#params),*
                ) -> #route_return_ty {
                    let __statum_next = self.#method_ident(#(#call_args),*) #await_tokens;
                    statum::__private::attest::<_, #route_type>(__statum_next)
                }
            })
        })
        .collect::<Vec<_>>();

    if methods.is_empty() {
        return quote! {};
    }

    let (impl_generics, _, _) = input_generics.split_for_impl();
    quote! {
        impl #impl_generics #target_type #where_clause {
            #(#methods)*
        }
    }
}

struct ViaMethodBinding<'a> {
    function: &'a TransitionFn,
    via_param_index: usize,
}

struct ViaBinderGroup<'a> {
    route: &'a ViaRoute,
    binding_ty: &'a Type,
    vis: &'a syn::Visibility,
    methods: Vec<ViaMethodBinding<'a>>,
}

fn generate_via_binders(
    input_generics: &syn::Generics,
    target_type: &Type,
    where_clause: Option<&syn::WhereClause>,
    tr_impl: &TransitionImpl,
) -> TokenStream {
    let groups = match collect_via_binder_groups(tr_impl) {
        Ok(groups) => groups,
        Err(err) => return err,
    };
    if groups.is_empty() {
        return quote! {};
    }

    let (impl_generics, _, _) = input_generics.split_for_impl();
    let type_generics = input_generics.clone();
    let adapter_tokens = groups.iter().enumerate().map(|(index, group)| {
        let binder_ident = format_ident!("from_{}", to_snake_case(&group.route.route_name));
        let adapter_ident = format_ident!(
            "__statum_{}_{}_{}_via_{}",
            to_snake_case(&tr_impl.machine_name),
            to_snake_case(&tr_impl.source_state),
            to_snake_case(&group.route.route_name),
            index,
        );
        let binding_ty = group.binding_ty;
        let route_type = &group.route.route_type;
        let vis = group.vis;
        let binder_impl = quote! {
            impl #impl_generics #target_type #where_clause {
                #vis fn #binder_ident(
                    self,
                    __statum_attested: statum::__private::Attested<#binding_ty, #route_type>,
                ) -> #adapter_ident #type_generics {
                    #adapter_ident {
                        machine: self,
                        attested: __statum_attested,
                    }
                }
            }
        };
        let adapter_methods = group.methods.iter().map(|binding| {
            let function = binding.function;
            let cfg_attrs = propagated_cfg_attrs(&tr_impl.attrs, &function.attrs);
            let vis = &function.vis;
            let asyncness = function.is_async.then_some(quote! { async });
            let await_tokens = function.is_async.then_some(quote! { .await });
            let method_ident = &function.name;
            let sig_generics = &function.sig_generics;
            let return_type = function
                .return_type
                .as_ref()
                .expect("validated transition return type");
            let params = function
                .parameters
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != binding.via_param_index)
                .map(|(_, param)| {
                    let binding_ident = &param.binding_ident;
                    let ty = &param.ty;
                    quote! { #binding_ident: #ty }
                })
                .collect::<Vec<_>>();
            let call_args = function
                .parameters
                .iter()
                .enumerate()
                .map(|(index, param)| {
                    if index == binding.via_param_index {
                        quote! { self.attested.into_inner() }
                    } else {
                        let binding_ident = &param.binding_ident;
                        quote! { #binding_ident }
                    }
                })
                .collect::<Vec<_>>();
            quote! {
                #(#cfg_attrs)*
                #vis #asyncness fn #method_ident #sig_generics (
                    self,
                    #(#params),*
                ) -> #return_type {
                    self.machine.#method_ident(#(#call_args),*) #await_tokens
                }
            }
        });
        quote! {
            #[doc(hidden)]
            #vis struct #adapter_ident #type_generics #where_clause {
                machine: #target_type,
                attested: statum::__private::Attested<#binding_ty, #route_type>,
            }

            impl #impl_generics #adapter_ident #type_generics #where_clause {
                #(#adapter_methods)*
            }

            #binder_impl
        }
    });

    quote! { #(#adapter_tokens)* }
}

fn collect_via_binder_groups<'a>(
    tr_impl: &'a TransitionImpl,
) -> Result<Vec<ViaBinderGroup<'a>>, TokenStream> {
    let mut groups = Vec::<ViaBinderGroup<'a>>::new();

    for function in &tr_impl.functions {
        let Some((via_param_index, via_param)) = function
            .parameters
            .iter()
            .enumerate()
            .find(|(_, param)| !param.via_routes.is_empty())
        else {
            continue;
        };

        for route in &via_param.via_routes {
            if let Some(existing) = groups.iter_mut().find(|group| {
                group.route.display_path == route.display_path
            }) {
                if existing.binding_ty.to_token_stream().to_string()
                    != via_param.ty.to_token_stream().to_string()
                {
                    let message = format!(
                        "Error: attested route `{}` is reused on `{}<{}>` with incompatible parameter types in v1.",
                        route.display_path, tr_impl.machine_name, tr_impl.source_state,
                    );
                    return Err(compile_error_at(via_param.span, &message));
                }
                if existing.vis.to_token_stream().to_string()
                    != function.vis.to_token_stream().to_string()
                {
                    let message = format!(
                        "Error: attested route `{}` is reused on `{}<{}>` with different visibilities in v1.",
                        route.display_path, tr_impl.machine_name, tr_impl.source_state,
                    );
                    return Err(compile_error_at(function.span, &message));
                }
                existing.methods.push(ViaMethodBinding {
                    function,
                    via_param_index,
                });
            } else {
                groups.push(ViaBinderGroup {
                    route,
                    binding_ty: &via_param.ty,
                    vis: &function.vis,
                    methods: vec![ViaMethodBinding {
                        function,
                        via_param_index,
                    }],
                });
            }
        }
    }

    Ok(groups)
}

fn route_marker_name(method_name: &str) -> String {
    let mut route = String::new();
    for part in method_name.split('_').filter(|part| !part.is_empty()) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            route.extend(first.to_uppercase());
            route.extend(chars.flat_map(char::to_lowercase));
        }
    }
    route
}

fn direct_return_state(function: &TransitionFn, target_type: &Type) -> Option<String> {
    let return_type = function.return_type.as_ref()?;
    let type_path = type_path(return_type)?;
    let segment = machine_segment_matching_target(&type_path.path, target_type)?;
    extract_machine_state_from_segment(segment).map(|(_, state, _)| state)
}

fn optional_lit_str_tokens(value: Option<&str>, span: Span) -> TokenStream {
    match value {
        Some(value) => {
            let lit = LitStr::new(value, span);
            quote! { Some(#lit) }
        }
        None => quote! { None },
    }
}

fn transition_presentation_metadata_tokens(
    presentation: &PresentationAttr,
) -> Result<TokenStream, TokenStream> {
    match presentation.metadata.as_deref() {
        Some(metadata_expr) => {
            let metadata = syn::parse_str::<syn::Expr>(metadata_expr)
                .map_err(|err| err.to_compile_error())?;
            Ok(quote! { #metadata })
        }
        None => Ok(quote! { () }),
    }
}

fn replace_machine_leaf_ident(
    target_type: &Type,
    replacement_ident: Ident,
) -> Result<Path, TokenStream> {
    let Type::Path(type_path) = target_type else {
        return Err(compile_error_at(
            target_type.span(),
            "Invalid #[transition] target type. Expected an impl target like `Machine<State>`.",
        ));
    };
    let mut replaced = type_path.path.clone();
    let Some(last_segment) = replaced.segments.last_mut() else {
        return Err(compile_error_at(
            target_type.span(),
            "Invalid #[transition] target type. Expected an impl target like `Machine<State>`.",
        ));
    };
    *last_segment = PathSegment::from(replacement_ident);

    Ok(replaced)
}

fn invalid_return_type_error(func: &TransitionFn, reason: &str) -> TokenStream {
    let func_name = &func.name;
    let return_type = func
        .return_type
        .as_ref()
        .map(|ty| ty.to_token_stream().to_string())
        .unwrap_or_else(|| "<none>".to_string());
    let machine_name = &func.machine_name;

    let message = format!(
        "Invalid transition return type for `{}<{}>::{func_name}`: {reason}.\n\n\
Expected:\n  fn {func_name}(self) -> {machine_name}<NextState>\n\n\
Actual:\n  {return_type}\n\n\
Help:\n  return `{machine_name}<NextState>` directly using the same machine path as the impl target, or wrap that same machine path in `::core::option::Option<...>`, `::core::result::Result<..., E>`, or `::statum::Branch<..., ...>` and build the next state with `self.transition()` or `self.transition_with(...)`.\n  Bare, aliased, or differently-qualified wrapper and machine paths are rejected because transition introspection only accepts exact syntactic return shapes."
        ,
        machine_name,
        func.source_state,
    );
    compile_error_at(func.return_type_span.unwrap_or(func.span), &message)
}

fn machine_return_signature(machine_name: &str) -> String {
    format!("{machine_name}<NextState>")
}

fn propagated_cfg_attrs(
    impl_attrs: &[syn::Attribute],
    function_attrs: &[syn::Attribute],
) -> Vec<syn::Attribute> {
    impl_attrs
        .iter()
        .chain(function_attrs.iter())
        .filter(|attr| {
            attr.path()
                .get_ident()
                .is_some_and(|ident| ident == "cfg" || ident == "cfg_attr")
        })
        .cloned()
        .collect()
}

fn transition_site_unique_suffix(
    tr_impl: &TransitionImpl,
    function: &TransitionFn,
    index: usize,
) -> String {
    let attrs = function
        .attrs
        .iter()
        .map(|attr| attr.to_token_stream().to_string())
        .collect::<Vec<_>>()
        .join("|");
    let return_type = function
        .return_type
        .as_ref()
        .map(|ty| ty.to_token_stream().to_string())
        .unwrap_or_default();
    let signature = format!(
        "{}::{}::{}::{}::{}::{}",
        tr_impl.machine_name,
        tr_impl.source_state,
        function.name,
        index,
        attrs,
        return_type,
    );

    format!("{:016x}", stable_hash(&signature))
}

fn transition_impl_unique_suffix(tr_impl: &TransitionImpl) -> String {
    let attrs = tr_impl
        .attrs
        .iter()
        .map(|attr| attr.to_token_stream().to_string())
        .collect::<Vec<_>>()
        .join("|");
    let functions = tr_impl
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| {
            let method_attrs = function
                .attrs
                .iter()
                .map(|attr| attr.to_token_stream().to_string())
                .collect::<Vec<_>>()
                .join("|");
            let return_type = function
                .return_type
                .as_ref()
                .map(|ty| ty.to_token_stream().to_string())
                .unwrap_or_default();
            format!(
                "{}::{}::{}::{}",
                function.name,
                index,
                method_attrs,
                return_type,
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    let signature = format!(
        "{}::{}::{}::{}",
        tr_impl.machine_name,
        tr_impl.source_state,
        attrs,
        functions,
    );

    format!("{:016x}", stable_hash(&signature))
}

fn relation_source_machine_descriptor(
    tr_impl: &TransitionImpl,
) -> Result<(String, String), TokenStream> {
    let Type::Path(type_path) = &tr_impl.target_type else {
        return Err(compile_error_at(
            tr_impl.target_type.span(),
            "Invalid #[transition] target type. Expected an impl target like `Machine<State>`.",
        ));
    };
    if type_path.qself.is_some() {
        return Err(compile_error_at(
            tr_impl.target_type.span(),
            "Invalid #[transition] target type. Qualified self types are not supported.",
        ));
    }

    let raw_segments = type_path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    if raw_segments.is_empty() {
        return Err(compile_error_at(
            tr_impl.target_type.span(),
            "Invalid #[transition] target type. Expected an impl target like `Machine<State>`.",
        ));
    }

    let current_module = split_module_path(&tr_impl.module_path);
    let resolved_segments = resolve_relation_source_segments(&raw_segments, &current_module);
    if resolved_segments.is_empty() {
        return Err(compile_error_at(
            tr_impl.target_type.span(),
            "Invalid #[transition] target type. Expected an impl target like `Machine<State>`.",
        ));
    }

    let rust_type_path = resolved_segments.join("::");
    let module_path = resolved_segments[..resolved_segments.len().saturating_sub(1)].join("::");
    Ok((module_path, rust_type_path))
}

fn split_module_path(module_path: &str) -> Vec<String> {
    module_path
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn resolve_relation_source_segments(
    raw_segments: &[String],
    current_module: &[String],
) -> Vec<String> {
    let Some(first) = raw_segments.first() else {
        return Vec::new();
    };

    match first.as_str() {
        "crate" => raw_segments[1..].to_vec(),
        "self" => current_module
            .iter()
            .cloned()
            .chain(raw_segments[1..].iter().cloned())
            .collect(),
        "super" => {
            let super_count = raw_segments
                .iter()
                .take_while(|segment| segment.as_str() == "super")
                .count();
            let keep = current_module.len().saturating_sub(super_count);
            current_module[..keep]
                .iter()
                .cloned()
                .chain(raw_segments[super_count..].iter().cloned())
                .collect()
        }
        _ if raw_segments.len() == 1 => current_module
            .iter()
            .cloned()
            .chain(raw_segments.iter().cloned())
            .collect(),
        _ => raw_segments.to_vec(),
    }
}

fn transition_param_relation_registrations(
    tr_impl: &TransitionImpl,
    function: &TransitionFn,
    function_index: usize,
    machine_module_path: &LitStr,
    machine_rust_type_path: &LitStr,
    cfg_attrs: &[syn::Attribute],
) -> Vec<TokenStream> {
    let source_state = LitStr::new(&tr_impl.source_state, function.name.span());
    let transition_name = LitStr::new(&function.name.to_string(), function.name.span());
    let generic_param_names = tr_impl
        .generic_params
        .iter()
        .cloned()
        .chain(function.generics.iter().map(ToString::to_string))
        .collect::<HashSet<_>>();
    let mut registrations = Vec::new();

    for (param_index, param) in function.parameters.iter().enumerate() {
        let targets = collect_relation_targets(&param.ty, &tr_impl.module_path);
        let param_name_tokens = match &param.name {
            Some(param_name) => quote! { Some(#param_name) },
            None => quote! { None },
        };
        let source_tokens = quote! {
            statum::__private::LinkedRelationSource::TransitionParam {
                state: #source_state,
                transition: #transition_name,
                param_index: #param_index,
                param_name: #param_name_tokens,
            }
        };
        registrations.extend(relation_registrations_for_targets(
            targets,
            source_tokens.clone(),
            machine_module_path,
            machine_rust_type_path,
            cfg_attrs,
            &format!(
                "{}::{}::{}::{}::param::{param_index}::function::{function_index}",
                tr_impl.module_path,
                tr_impl.machine_name,
                tr_impl.source_state,
                function.name
            ),
            &generic_param_names,
        ));

        if let Some((target_machine_path, target_state)) =
            exact_direct_machine_target(&param.ty, &tr_impl.module_path)
        {
            for (route_index, route) in param.via_routes.iter().enumerate() {
                let registration_ident = format_ident!(
                    "__STATUM_LINKED_RELATION_{:016X}",
                    stable_hash(&format!(
                        "{}::{}::{}::{}::param::{param_index}::route::{route_index}",
                        tr_impl.module_path,
                        tr_impl.machine_name,
                        tr_impl.source_state,
                        function.name,
                    ))
                );
                let route_name = LitStr::new(&route.route_name, function.name.span());
                let via_module_path = LitStr::new(&route.via_module_path, function.name.span());
                let route_id = route.route_id;
                let target_machine_path_tokens = target_machine_path.iter().map(|segment| {
                    let segment = LitStr::new(segment, function.name.span());
                    quote! { #segment }
                });
                let target_state = LitStr::new(&target_state, function.name.span());
                registrations.push(quote! {
                    #(#cfg_attrs)*
                    #[doc(hidden)]
                    #[statum::__private::linkme::distributed_slice(statum::__private::__STATUM_LINKED_RELATIONS)]
                    #[linkme(crate = statum::__private::linkme)]
                    static #registration_ident: statum::__private::LinkedRelationDescriptor =
                        statum::__private::LinkedRelationDescriptor {
                            machine: statum::MachineDescriptor {
                                module_path: #machine_module_path,
                                rust_type_path: #machine_rust_type_path,
                            },
                            kind: statum::__private::LinkedRelationKind::TransitionParam,
                            source: #source_tokens,
                            basis: statum::__private::LinkedRelationBasis::ViaDeclaration,
                            target: statum::__private::LinkedRelationTarget::AttestedRoute {
                                via_module_path: #via_module_path,
                                route_name: #route_name,
                                route_id: #route_id,
                                machine_path: &[#(#target_machine_path_tokens),*],
                                state: #target_state,
                            },
                        };
                });
            }
        }
    }

    registrations
}

fn exact_direct_machine_target(
    ty: &Type,
    source_module_path: &str,
) -> Option<(Vec<String>, String)> {
    match collect_relation_targets(ty, source_module_path).as_slice() {
        [RelationTargetCandidate::DirectMachine {
            machine_path,
            state_name,
        }] => Some((machine_path.clone(), state_name.clone())),
        _ => None,
    }
}

fn relation_registrations_for_targets(
    targets: Vec<RelationTargetCandidate>,
    source_tokens: TokenStream,
    machine_module_path: &LitStr,
    machine_rust_type_path: &LitStr,
    cfg_attrs: &[syn::Attribute],
    key_prefix: &str,
    generic_param_names: &HashSet<String>,
) -> Vec<TokenStream> {
    targets
        .into_iter()
        .filter(|target| !references_generic_param(target, generic_param_names))
        .enumerate()
        .map(|(index, target)| {
            let registration_ident = format_ident!(
                "__STATUM_LINKED_RELATION_{:016X}",
                stable_hash(&format!("{key_prefix}::{index}::registration"))
            );
            let (basis_tokens, target_tokens, helper_tokens) = match target {
                RelationTargetCandidate::DirectMachine {
                    machine_path,
                    state_name,
                } => {
                    let machine_path = machine_path.iter().map(|segment| {
                        let segment = LitStr::new(segment, Span::call_site());
                        quote! { #segment }
                    });
                    let state_name = LitStr::new(&state_name, Span::call_site());
                    (
                        quote! { statum::__private::LinkedRelationBasis::DirectTypeSyntax },
                        quote! {
                            statum::__private::LinkedRelationTarget::DirectMachine {
                                machine_path: &[#(#machine_path),*],
                                state: #state_name,
                            }
                        },
                        quote! {},
                    )
                }
                RelationTargetCandidate::DeclaredReferenceType { ty } => {
                    let helper_ident = format_ident!(
                        "__statum_transition_relation_type_name_{:016x}",
                        stable_hash(&format!(
                            "{key_prefix}::{index}::{}",
                            ty.to_token_stream()
                        ))
                    );
                    (
                        quote! { statum::__private::LinkedRelationBasis::DeclaredReferenceType },
                        quote! {
                            statum::__private::LinkedRelationTarget::DeclaredReferenceType {
                                resolved_type_name: #helper_ident,
                            }
                        },
                        quote! {
                            #[doc(hidden)]
                            fn #helper_ident() -> &'static str {
                                ::core::any::type_name::<#ty>()
                            }
                        },
                    )
                }
            };

            quote! {
                #helper_tokens

                #(#cfg_attrs)*
                #[doc(hidden)]
                #[statum::__private::linkme::distributed_slice(statum::__private::__STATUM_LINKED_RELATIONS)]
                #[linkme(crate = statum::__private::linkme)]
                static #registration_ident: statum::__private::LinkedRelationDescriptor =
                    statum::__private::LinkedRelationDescriptor {
                        machine: statum::MachineDescriptor {
                            module_path: #machine_module_path,
                            rust_type_path: #machine_rust_type_path,
                        },
                        kind: statum::__private::LinkedRelationKind::TransitionParam,
                        source: #source_tokens,
                        basis: #basis_tokens,
                        target: #target_tokens,
                    };
            }
        })
        .collect()
}

fn references_generic_param(
    target: &RelationTargetCandidate,
    generic_param_names: &HashSet<String>,
) -> bool {
    let RelationTargetCandidate::DeclaredReferenceType { ty } = target else {
        return false;
    };

    leading_type_ident(ty)
        .is_some_and(|ident| ident == "Self" || generic_param_names.contains(&ident.to_string()))
}

fn stable_hash(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn compile_error_at(span: Span, message: &str) -> TokenStream {
    let message = LitStr::new(message, span);
    quote::quote_spanned! { span =>
        compile_error!(#message);
    }
}

/// Attempts to parse `ty` into the form:
///
///   - the same machine path as the impl target, with a different state marker
///   - `::core::option::Option<...>` or `::std::option::Option<...>`
///   - `::core::result::Result<..., E>` or `::std::result::Result<..., E>`
///   - `::statum::Branch<..., ...>` or `::statum_core::Branch<..., ...>`
///
/// On success, returns (`"Machine"`, `"SomeState"`).
pub fn parse_machine_and_state(ty: &Type, target_type: &Type) -> Option<(String, String)> {
    parse_primary_machine_and_state(ty, target_type)
}

/// Attempts to parse the primary visible next state from `ty`.
///
/// This preserves transition helper behavior by following the first generic
/// argument through supported wrappers until it reaches the same machine path
/// used by the impl target.
pub fn parse_primary_machine_and_state(ty: &Type, target_type: &Type) -> Option<(String, String)> {
    let mut current = ty;
    loop {
        match classify_primary_return_wrapper(current, target_type)? {
            PrimaryReturnWrapper::Machine(segment) => {
                return extract_machine_state_from_segment(segment)
                    .map(|(machine, state, _)| (machine, state));
            }
            PrimaryReturnWrapper::Option(inner)
            | PrimaryReturnWrapper::Result(inner)
            | PrimaryReturnWrapper::Branch(inner) => {
                current = inner;
            }
        }
    }
}

/// Collects every `Machine<State>` target mentioned in supported wrapper trees.
///
/// This is used for exact branch introspection and intentionally inspects both
/// sides of `Result<T, E>` while still ignoring arbitrary non-machine payloads.
pub fn collect_machine_and_states(ty: &Type, target_type: &Type) -> Vec<(String, String)> {
    let mut targets = Vec::new();
    collect_machine_targets(ty, target_type, &mut targets);
    targets
}

enum PrimaryReturnWrapper<'a> {
    Machine(&'a syn::PathSegment),
    Option(&'a Type),
    Result(&'a Type),
    Branch(&'a Type),
}

#[derive(Clone, Copy)]
enum SupportedWrapper {
    Option,
    Result,
    Branch,
}

fn classify_primary_return_wrapper<'a>(
    ty: &'a Type,
    target_type: &Type,
) -> Option<PrimaryReturnWrapper<'a>> {
    let type_path = type_path(ty)?;

    if let Some(segment) = machine_segment_matching_target(&type_path.path, target_type) {
        return Some(PrimaryReturnWrapper::Machine(segment));
    }

    let segment = type_path.path.segments.last()?;
    match supported_wrapper(&type_path.path)? {
        SupportedWrapper::Option => {
            extract_first_generic_type_ref(&segment.arguments).map(PrimaryReturnWrapper::Option)
        }
        SupportedWrapper::Result => {
            extract_first_generic_type_ref(&segment.arguments).map(PrimaryReturnWrapper::Result)
        }
        SupportedWrapper::Branch => {
            extract_first_generic_type_ref(&segment.arguments).map(PrimaryReturnWrapper::Branch)
        }
    }
}

fn collect_machine_targets(ty: &Type, target_type: &Type, targets: &mut Vec<(String, String)>) {
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

    match supported_wrapper(&type_path.path) {
        Some(SupportedWrapper::Option) => {
            if let Some(inner) = extract_first_generic_type_ref(&segment.arguments) {
                collect_machine_targets(inner, target_type, targets);
            }
        }
        Some(SupportedWrapper::Result | SupportedWrapper::Branch) => {
            if let Some(types) = extract_generic_type_refs(&segment.arguments) {
                for inner in types {
                    collect_machine_targets(inner, target_type, targets);
                }
            }
        }
        None => {}
    }
}

fn push_unique_target(targets: &mut Vec<(String, String)>, machine: String, state: String) {
    if !targets.iter().any(|(existing_machine, existing_state)| {
        existing_machine == &machine && existing_state == &state
    }) {
        targets.push((machine, state));
    }
}

fn type_path(ty: &Type) -> Option<&TypePath> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    type_path.qself.is_none().then_some(type_path)
}

fn machine_segment_matching_target<'a>(
    candidate_path: &'a syn::Path,
    target_type: &Type,
) -> Option<&'a syn::PathSegment> {
    let target_path = &type_path(target_type)?.path;
    path_matches_target_machine(candidate_path, target_path)
        .then(|| candidate_path.segments.last())
        .flatten()
}

fn path_matches_target_machine(candidate: &syn::Path, target: &syn::Path) -> bool {
    if candidate.leading_colon.is_some() != target.leading_colon.is_some() {
        return false;
    }
    if candidate.segments.len() != target.segments.len() {
        return false;
    }

    let last_index = candidate.segments.len().saturating_sub(1);
    for (index, (candidate_segment, target_segment)) in
        candidate.segments.iter().zip(target.segments.iter()).enumerate()
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

fn supported_wrapper(path: &syn::Path) -> Option<SupportedWrapper> {
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

fn extract_machine_state_from_segment(segment: &syn::PathSegment) -> Option<(String, String, Span)> {
    extract_machine_generic(&segment.arguments, &segment.ident.to_string())
}

fn extract_machine_generic(args: &PathArguments, machine_name: &str) -> Option<(String, String, Span)> {
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

fn extract_generic_type_refs(args: &PathArguments) -> Option<Vec<&Type>> {
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
        collect_machine_and_states, extract_impl_machine_and_state, parse_machine_and_state,
        parse_primary_machine_and_state,
    };
    use syn::Type;

    fn parse_type(source: &str) -> Type {
        syn::parse_str(source).expect("valid type")
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
    fn impl_target_rejects_qualified_state_paths() {
        let ty = parse_type("Machine<crate::Draft>");
        assert!(extract_impl_machine_and_state(&ty).is_none());
    }
}
