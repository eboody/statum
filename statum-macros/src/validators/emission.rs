use std::collections::{HashMap, HashSet};

use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::visit_mut::{self, VisitMut};
use syn::Type;

use crate::validators::{ValidatorMethodSpec, signatures::ValidatorReturnKind};

pub(super) fn validator_support_macro_ident(machine_name: &str) -> syn::Ident {
    format_ident!(
        "__statum_expand_{}_validators",
        crate::to_snake_case(machine_name)
    )
}

pub(super) fn emit_validator_methods_impl(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let parsed = syn::parse_macro_input!(input as ValidatorMethodsInput);
    generate_validator_methods_impl(parsed).into()
}

pub(super) fn generate_validator_build_variant_macro(
    machine_name: &str,
    validator_methods: &[ValidatorMethodSpec],
) -> proc_macro2::TokenStream {
    generate_validator_variant_macro(machine_name, validator_methods, ValidatorVariantMacro::Build)
}

pub(super) fn generate_validator_report_variant_macro(
    machine_name: &str,
    validator_methods: &[ValidatorMethodSpec],
) -> proc_macro2::TokenStream {
    generate_validator_variant_macro(machine_name, validator_methods, ValidatorVariantMacro::Report)
}

enum ValidatorVariantMacro {
    Build,
    Report,
}

struct ValidatorMethodsInput {
    persisted: Type,
    extra_generics: ValidatorMethodExtraGenerics,
    fields: Vec<ValidatorMethodField>,
    validator_methods: Vec<syn::ImplItemFn>,
}

struct ValidatorMethodExtraGenerics {
    params: Vec<syn::GenericParam>,
    where_predicates: Vec<syn::WherePredicate>,
}

struct ValidatorMethodField {
    name: syn::Ident,
    ty: syn::Type,
}

impl Parse for ValidatorMethodsInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let persisted = parse_named_value(input, "persisted")?;
        let extra_generics = parse_named_value(input, "extra_generics")?;
        let fields = parse_named_fields(input, "fields")?;
        let validator_methods = parse_named_validator_methods(input, "validator_methods")?;
        Ok(Self {
            persisted,
            extra_generics,
            fields,
            validator_methods,
        })
    }
}

impl Parse for ValidatorMethodExtraGenerics {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);
        let params = parse_named_generic_params(&content, "params")?;
        let where_predicates = parse_named_where_predicates(&content, "where_predicates")?;
        Ok(Self {
            params,
            where_predicates,
        })
    }
}

impl Parse for ValidatorMethodField {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);
        let name = parse_named_value(&content, "name")?;
        let ty = parse_named_value(&content, "ty")?;
        Ok(Self { name, ty })
    }
}

fn parse_braced_generic_param(input: ParseStream<'_>) -> syn::Result<syn::GenericParam> {
    let content;
    syn::braced!(content in input);
    content.parse()
}

fn parse_braced_where_predicate(input: ParseStream<'_>) -> syn::Result<syn::WherePredicate> {
    let content;
    syn::braced!(content in input);
    content.parse()
}

fn parse_named_value<T: Parse>(input: ParseStream<'_>, expected: &str) -> syn::Result<T> {
    let ident: syn::Ident = input.parse()?;
    if ident != expected {
        return Err(syn::Error::new(
            ident.span(),
            format!("expected `{expected}`"),
        ));
    }
    input.parse::<syn::Token![=]>()?;
    let value = input.parse()?;
    if input.peek(syn::Token![,]) {
        input.parse::<syn::Token![,]>()?;
    }
    Ok(value)
}

fn parse_named_fields(
    input: ParseStream<'_>,
    expected: &str,
) -> syn::Result<Vec<ValidatorMethodField>> {
    parse_named_bracketed_list(input, expected, ValidatorMethodField::parse)
}

fn parse_named_validator_methods(
    input: ParseStream<'_>,
    expected: &str,
) -> syn::Result<Vec<syn::ImplItemFn>> {
    parse_named_bracketed_list(input, expected, syn::ImplItemFn::parse)
}

fn parse_named_generic_params(
    input: ParseStream<'_>,
    expected: &str,
) -> syn::Result<Vec<syn::GenericParam>> {
    parse_named_bracketed_list(input, expected, parse_braced_generic_param)
}

fn parse_named_where_predicates(
    input: ParseStream<'_>,
    expected: &str,
) -> syn::Result<Vec<syn::WherePredicate>> {
    parse_named_bracketed_list(input, expected, parse_braced_where_predicate)
}

fn parse_named_bracketed_list<T>(
    input: ParseStream<'_>,
    expected: &str,
    parser: fn(ParseStream<'_>) -> syn::Result<T>,
) -> syn::Result<Vec<T>> {
    let ident: syn::Ident = input.parse()?;
    if ident != expected {
        return Err(syn::Error::new(
            ident.span(),
            format!("expected `{expected}`"),
        ));
    }
    input.parse::<syn::Token![=]>()?;
    let content;
    syn::bracketed!(content in input);
    let items = content.parse_terminated(parser, syn::Token![,])?;
    if input.peek(syn::Token![,]) {
        input.parse::<syn::Token![,]>()?;
    }
    Ok(items.into_iter().collect())
}

fn generate_validator_methods_impl(input: ValidatorMethodsInput) -> proc_macro2::TokenStream {
    let ValidatorMethodsInput {
        persisted,
        extra_generics,
        fields,
        validator_methods,
    } = input;
    let rewritten_methods = validator_methods
        .into_iter()
        .map(|method| rewrite_validator_method(method, &extra_generics, &fields))
        .collect::<Vec<_>>();

    quote! {
        impl #persisted {
            #(#rewritten_methods)*
        }
    }
}

fn rewrite_validator_method(
    mut method: syn::ImplItemFn,
    extra_generics: &ValidatorMethodExtraGenerics,
    fields: &[ValidatorMethodField],
) -> syn::ImplItemFn {
    method.attrs.push(syn::parse_quote!(#[allow(clippy::ptr_arg)]));
    method.sig.generics.params.extend(extra_generics.params.clone());
    if !extra_generics.where_predicates.is_empty() {
        let where_clause = method.sig.generics.make_where_clause();
        where_clause
            .predicates
            .extend(extra_generics.where_predicates.clone());
    }
    let field_bindings = fields
        .iter()
        .map(|field| {
            (
                field.name.to_string(),
                syn::Ident::new(
                    &format!("__statum_machine_field_{}", field.name),
                    proc_macro2::Span::call_site(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    rewrite_validator_body(&mut method.block, &field_bindings);
    for field in fields {
        let field_name = field_bindings
            .get(&field.name.to_string())
            .expect("field binding present");
        let field_ty = &field.ty;
        method
            .sig
            .inputs
            .push(syn::parse_quote!(#field_name: &#field_ty));
    }
    method
}

fn rewrite_validator_body(
    block: &mut syn::Block,
    field_bindings: &HashMap<String, syn::Ident>,
) {
    let mut rewriter = ValidatorFieldRewriter::new(field_bindings);
    rewriter.visit_block_mut(block);
}

struct ValidatorFieldRewriter<'a> {
    field_bindings: &'a HashMap<String, syn::Ident>,
    shadowed: Vec<HashSet<String>>,
}

impl<'a> ValidatorFieldRewriter<'a> {
    fn new(field_bindings: &'a HashMap<String, syn::Ident>) -> Self {
        Self {
            field_bindings,
            shadowed: vec![HashSet::new()],
        }
    }

    fn push_scope(&mut self) {
        self.shadowed.push(HashSet::new());
    }

    fn pop_scope(&mut self) {
        self.shadowed.pop();
    }

    fn insert_bindings_from_pat(&mut self, pat: &syn::Pat) {
        let mut bindings = Vec::new();
        collect_pat_idents(pat, &mut bindings);
        if let Some(scope) = self.shadowed.last_mut() {
            for ident in bindings {
                scope.insert(ident.to_string());
            }
        }
    }

    fn is_shadowed(&self, ident: &str) -> bool {
        self.shadowed
            .iter()
            .rev()
            .any(|scope| scope.contains(ident))
    }
}

impl VisitMut for ValidatorFieldRewriter<'_> {
    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        self.push_scope();
        for stmt in &mut block.stmts {
            match stmt {
                syn::Stmt::Local(local) => {
                    if let Some(init) = &mut local.init {
                        self.visit_expr_mut(&mut init.expr);
                        if let Some((_, diverge)) = &mut init.diverge {
                            self.visit_expr_mut(diverge);
                        }
                    }
                    self.insert_bindings_from_pat(&local.pat);
                }
                _ => visit_mut::visit_stmt_mut(self, stmt),
            }
        }
        self.pop_scope();
    }

    fn visit_expr_closure_mut(&mut self, closure: &mut syn::ExprClosure) {
        self.push_scope();
        for input in &closure.inputs {
            self.insert_bindings_from_pat(input);
        }
        self.visit_expr_mut(&mut closure.body);
        self.pop_scope();
    }

    fn visit_expr_for_loop_mut(&mut self, expr: &mut syn::ExprForLoop) {
        self.visit_expr_mut(&mut expr.expr);
        self.push_scope();
        self.insert_bindings_from_pat(&expr.pat);
        self.visit_block_mut(&mut expr.body);
        self.pop_scope();
    }

    fn visit_arm_mut(&mut self, arm: &mut syn::Arm) {
        self.push_scope();
        self.insert_bindings_from_pat(&arm.pat);
        if let Some((_, guard)) = &mut arm.guard {
            self.visit_expr_mut(guard);
        }
        self.visit_expr_mut(&mut arm.body);
        self.pop_scope();
    }

    fn visit_expr_if_mut(&mut self, expr: &mut syn::ExprIf) {
        if let syn::Expr::Let(expr_let) = &mut *expr.cond {
            self.visit_expr_mut(&mut expr_let.expr);
            self.push_scope();
            self.insert_bindings_from_pat(&expr_let.pat);
            self.visit_block_mut(&mut expr.then_branch);
            self.pop_scope();
            if let Some((_, else_branch)) = &mut expr.else_branch {
                self.visit_expr_mut(else_branch);
            }
            return;
        }

        visit_mut::visit_expr_if_mut(self, expr);
    }

    fn visit_expr_while_mut(&mut self, expr: &mut syn::ExprWhile) {
        if let syn::Expr::Let(expr_let) = &mut *expr.cond {
            self.visit_expr_mut(&mut expr_let.expr);
            self.push_scope();
            self.insert_bindings_from_pat(&expr_let.pat);
            self.visit_block_mut(&mut expr.body);
            self.pop_scope();
            return;
        }

        visit_mut::visit_expr_while_mut(self, expr);
    }

    fn visit_expr_path_mut(&mut self, expr: &mut syn::ExprPath) {
        if expr.qself.is_none() && expr.path.segments.len() == 1 {
            let ident = &expr.path.segments[0].ident;
            if !self.is_shadowed(&ident.to_string())
                && let Some(replacement) = self.field_bindings.get(&ident.to_string())
            {
                expr.path = syn::parse_quote!(#replacement);
                return;
            }
        }

        visit_mut::visit_expr_path_mut(self, expr);
    }
}

fn collect_pat_idents(pat: &syn::Pat, out: &mut Vec<syn::Ident>) {
    match pat {
        syn::Pat::Ident(pat_ident) => out.push(pat_ident.ident.clone()),
        syn::Pat::Or(pat_or) => {
            for case in &pat_or.cases {
                collect_pat_idents(case, out);
            }
        }
        syn::Pat::Paren(pat_paren) => collect_pat_idents(&pat_paren.pat, out),
        syn::Pat::Reference(pat_reference) => collect_pat_idents(&pat_reference.pat, out),
        syn::Pat::Slice(pat_slice) => {
            for elem in &pat_slice.elems {
                collect_pat_idents(elem, out);
            }
        }
        syn::Pat::Struct(pat_struct) => {
            for field in &pat_struct.fields {
                collect_pat_idents(&field.pat, out);
            }
        }
        syn::Pat::Tuple(pat_tuple) => {
            for elem in &pat_tuple.elems {
                collect_pat_idents(elem, out);
            }
        }
        syn::Pat::TupleStruct(pat_tuple_struct) => {
            for elem in &pat_tuple_struct.elems {
                collect_pat_idents(elem, out);
            }
        }
        syn::Pat::Type(pat_type) => collect_pat_idents(&pat_type.pat, out),
        _ => {}
    }
}

fn generate_validator_variant_macro(
    machine_name: &str,
    validator_methods: &[ValidatorMethodSpec],
    kind: ValidatorVariantMacro,
) -> proc_macro2::TokenStream {
    let macro_ident = match kind {
        ValidatorVariantMacro::Build => format_ident!(
            "__statum_emit_{}_validator_build_variant",
            crate::to_snake_case(machine_name)
        ),
        ValidatorVariantMacro::Report => format_ident!(
            "__statum_emit_{}_validator_report_variant",
            crate::to_snake_case(machine_name)
        ),
    };
    let arms = validator_methods
        .iter()
        .map(|method| generate_validator_variant_arm(method, &kind))
        .collect::<Vec<_>>();
    let fallback_arm = match kind {
        ValidatorVariantMacro::Build => quote! {
            (
                persisted = $persisted:ident,
                machine = $machine:ident,
                state_family = $state_family:ident,
                machine_module = $machine_module:ident,
                machine_builder = $machine_builder:path,
                variant = $variant:ident,
                state_variant = $state_variant:path,
                validator = $validator:ident,
                data = $data:ty,
                has_data = $has_data:tt,
                fields = [$( { name = $field:ident, ty = $field_ty:ty } ),* $(,)?],
            ) => {};
        },
        ValidatorVariantMacro::Report => quote! {
            (
                persisted = $persisted:ident,
                attempts = $attempts:ident,
                machine = $machine:ident,
                state_family = $state_family:ident,
                machine_module = $machine_module:ident,
                machine_builder = $machine_builder:path,
                variant = $variant:ident,
                state_variant = $state_variant:path,
                validator = $validator:ident,
                data = $data:ty,
                has_data = $has_data:tt,
                fields = [$( { name = $field:ident, ty = $field_ty:ty } ),* $(,)?],
            ) => {};
        },
    };

    quote! {
        #[doc(hidden)]
        macro_rules! #macro_ident {
            #(#arms)*
            #fallback_arm
        }
    }
}

fn generate_validator_variant_arm(
    method: &ValidatorMethodSpec,
    kind: &ValidatorVariantMacro,
) -> proc_macro2::TokenStream {
    let validator_ident = &method.validator_ident;
    let actual_ok_type = &method.actual_ok_type;
    let payload_check_fn_ident = format_ident!("__statum_payload_for_{}", validator_ident);
    let await_token = if method.is_async {
        quote! { .await }
    } else {
        quote! {}
    };
    let call = quote! { $persisted.#validator_ident($( &$field ),*)#await_token };

    match kind {
        ValidatorVariantMacro::Build => {
            let with_data_body = quote! {
                if let Ok(__statum_state_data) = #call {
                    return Ok($state_variant(
                        <$machine_builder>::builder()
                            $( .$field($field) )*
                            .state_data(__statum_state_data)
                            .build()
                    ));
                }
            };
            let without_data_body = quote! {
                if #call.is_ok() {
                    return Ok($state_variant(
                        <$machine_builder>::builder()
                            $( .$field($field) )*
                            .build()
                    ));
                }
            };

            quote! {
                (
                    persisted = $persisted:ident,
                    machine = $machine:ident,
                    state_family = $state_family:ident,
                    machine_module = $machine_module:ident,
                    machine_builder = $machine_builder:path,
                    variant = $variant:ident,
                    state_variant = $state_variant:path,
                    validator = #validator_ident,
                    data = $data:ty,
                    has_data = true,
                    fields = [$( { name = $field:ident, ty = $field_ty:ty } ),* $(,)?],
                ) => {
                    {
                        fn #payload_check_fn_ident(_: core::option::Option<$data>) {}
                        #payload_check_fn_ident(core::option::Option::<#actual_ok_type>::None);
                        #with_data_body
                    }
                };
                (
                    persisted = $persisted:ident,
                    machine = $machine:ident,
                    state_family = $state_family:ident,
                    machine_module = $machine_module:ident,
                    machine_builder = $machine_builder:path,
                    variant = $variant:ident,
                    state_variant = $state_variant:path,
                    validator = #validator_ident,
                    data = $data:ty,
                    has_data = false,
                    fields = [$( { name = $field:ident, ty = $field_ty:ty } ),* $(,)?],
                ) => {
                    {
                        fn #payload_check_fn_ident(_: core::option::Option<$data>) {}
                        #payload_check_fn_ident(core::option::Option::<#actual_ok_type>::None);
                        #without_data_body
                    }
                };
            }
        }
        ValidatorVariantMacro::Report => {
            let with_data_body = match method.return_kind {
                ValidatorReturnKind::Plain => quote! {
                    match #call {
                        Ok(__statum_state_data) => {
                            $attempts.push(statum::RebuildAttempt {
                                validator: stringify!(#validator_ident),
                                target_state: stringify!($variant),
                                matched: true,
                                reason_key: core::option::Option::None,
                                message: core::option::Option::None,
                            });
                            return statum::RebuildReport {
                                attempts: $attempts,
                                result: Ok($state_variant(
                                    <$machine_builder>::builder()
                                        $( .$field($field) )*
                                        .state_data(__statum_state_data)
                                        .build()
                                )),
                            };
                        }
                        Err(_) => $attempts.push(statum::RebuildAttempt {
                            validator: stringify!(#validator_ident),
                            target_state: stringify!($variant),
                            matched: false,
                            reason_key: core::option::Option::None,
                            message: core::option::Option::None,
                        }),
                    }
                },
                ValidatorReturnKind::Diagnostic => quote! {
                    match #call {
                        Ok(__statum_state_data) => {
                            $attempts.push(statum::RebuildAttempt {
                                validator: stringify!(#validator_ident),
                                target_state: stringify!($variant),
                                matched: true,
                                reason_key: core::option::Option::None,
                                message: core::option::Option::None,
                            });
                            return statum::RebuildReport {
                                attempts: $attempts,
                                result: Ok($state_variant(
                                    <$machine_builder>::builder()
                                        $( .$field($field) )*
                                        .state_data(__statum_state_data)
                                        .build()
                                )),
                            };
                        }
                        Err(__statum_rejection) => $attempts.push(statum::RebuildAttempt {
                            validator: stringify!(#validator_ident),
                            target_state: stringify!($variant),
                            matched: false,
                            reason_key: core::option::Option::Some(__statum_rejection.reason_key),
                            message: __statum_rejection.message.clone(),
                        }),
                    }
                },
            };
            let without_data_body = match method.return_kind {
                ValidatorReturnKind::Plain => quote! {
                    if #call.is_ok() {
                        $attempts.push(statum::RebuildAttempt {
                            validator: stringify!(#validator_ident),
                            target_state: stringify!($variant),
                            matched: true,
                            reason_key: core::option::Option::None,
                            message: core::option::Option::None,
                        });
                        return statum::RebuildReport {
                            attempts: $attempts,
                            result: Ok($state_variant(
                                <$machine_builder>::builder()
                                    $( .$field($field) )*
                                    .build()
                            )),
                        };
                    }

                    $attempts.push(statum::RebuildAttempt {
                        validator: stringify!(#validator_ident),
                        target_state: stringify!($variant),
                        matched: false,
                        reason_key: core::option::Option::None,
                        message: core::option::Option::None,
                    });
                },
                ValidatorReturnKind::Diagnostic => quote! {
                    match #call {
                        Ok(()) => {
                            $attempts.push(statum::RebuildAttempt {
                                validator: stringify!(#validator_ident),
                                target_state: stringify!($variant),
                                matched: true,
                                reason_key: core::option::Option::None,
                                message: core::option::Option::None,
                            });
                            return statum::RebuildReport {
                                attempts: $attempts,
                                result: Ok($state_variant(
                                    <$machine_builder>::builder()
                                        $( .$field($field) )*
                                        .build()
                                )),
                            };
                        }
                        Err(__statum_rejection) => {
                            $attempts.push(statum::RebuildAttempt {
                                validator: stringify!(#validator_ident),
                                target_state: stringify!($variant),
                                matched: false,
                                reason_key: core::option::Option::Some(__statum_rejection.reason_key),
                                message: __statum_rejection.message.clone(),
                            });
                        }
                    }
                },
            };

            quote! {
                (
                    persisted = $persisted:ident,
                    attempts = $attempts:ident,
                    machine = $machine:ident,
                    state_family = $state_family:ident,
                    machine_module = $machine_module:ident,
                    machine_builder = $machine_builder:path,
                    variant = $variant:ident,
                    state_variant = $state_variant:path,
                    validator = #validator_ident,
                    data = $data:ty,
                    has_data = true,
                    fields = [$( { name = $field:ident, ty = $field_ty:ty } ),* $(,)?],
                ) => {
                    {
                        fn #payload_check_fn_ident(_: core::option::Option<$data>) {}
                        #payload_check_fn_ident(core::option::Option::<#actual_ok_type>::None);
                        #with_data_body
                    }
                };
                (
                    persisted = $persisted:ident,
                    attempts = $attempts:ident,
                    machine = $machine:ident,
                    state_family = $state_family:ident,
                    machine_module = $machine_module:ident,
                    machine_builder = $machine_builder:path,
                    variant = $variant:ident,
                    state_variant = $state_variant:path,
                    validator = #validator_ident,
                    data = $data:ty,
                    has_data = false,
                    fields = [$( { name = $field:ident, ty = $field_ty:ty } ),* $(,)?],
                ) => {
                    {
                        fn #payload_check_fn_ident(_: core::option::Option<$data>) {}
                        #payload_check_fn_ident(core::option::Option::<#actual_ok_type>::None);
                        #without_data_body
                    }
                };
            }
        }
    }
}
