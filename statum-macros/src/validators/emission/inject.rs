use syn::{Generics, Ident, ImplItem, ImplItemFn, Type};

pub(crate) fn inject_machine_fields(
    methods: &[ImplItem],
    parsed_fields: &[(Ident, Type)],
    extra_machine_generics: &Generics,
) -> Result<Vec<ImplItem>, proc_macro2::TokenStream> {
    Ok(methods
        .iter()
        .map(|item| {
            if let ImplItem::Fn(func) = item {
                let fn_name = &func.sig.ident;

                if super::super::signatures::validator_state_name_from_ident(fn_name).is_some() {
                    let mut new_inputs = func.sig.inputs.clone();

                    for (ident, ty) in parsed_fields.iter() {
                        new_inputs.push(syn::FnArg::Typed(syn::parse_quote! { #ident: &#ty }));
                    }

                    let mut generics = func.sig.generics.clone();
                    if !extra_machine_generics.params.is_empty() {
                        if generics.lt_token.is_none() {
                            generics.lt_token = Some(Default::default());
                            generics.gt_token = Some(Default::default());
                        }
                        generics
                            .params
                            .extend(extra_machine_generics.params.iter().cloned());
                    }
                    if let Some(extra_where_clause) = &extra_machine_generics.where_clause {
                        let where_clause = generics.make_where_clause();
                        where_clause
                            .predicates
                            .extend(extra_where_clause.predicates.iter().cloned());
                    }

                    let mut attrs = func.attrs.clone();
                    attrs.push(syn::parse_quote!(#[allow(clippy::ptr_arg)]));
                    let body = &func.block;

                    return ImplItem::Fn(ImplItemFn {
                        attrs,
                        sig: syn::Signature {
                            inputs: new_inputs,
                            generics,
                            ..func.sig.clone()
                        },
                        block: body.clone(),
                        ..func.clone()
                    });
                }
            }
            item.clone()
        })
        .collect())
}
