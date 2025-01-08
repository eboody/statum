use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parser, parse_macro_input, DeriveInput, Fields};

#[proc_macro_attribute]
pub fn state(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let vis = &input.vis;
    let name = &input.ident;

    let states = match &input.data {
        syn::Data::Enum(data_enum) => data_enum.variants.iter().map(|variant| {
            let variant_ident = &variant.ident;
            let variant_fields = &variant.fields;

            match variant_fields {
                // Handle tuple variant with one field
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                    let field_type = &fields.unnamed.first().unwrap().ty;
                    quote! {
                        #vis struct #variant_ident(#field_type);
                        impl #name for #variant_ident {
                            type Data = #field_type;
                        }
                    }
                }
                // Handle unit variant (no fields)
                Fields::Unit => {
                    quote! {
                        #vis struct #variant_ident;
                        impl #name for #variant_ident {
                            type Data = ();
                        }
                    }
                }
                // Error on other variants
                _ => panic!("Variants must either be unit variants or single-field tuple variants"),
            }
        }),
        _ => panic!("state attribute can only be used on enums"),
    };

    let expanded = quote! {
        #vis trait #name {
            type Data;
        }
        #(#states)*
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn context(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as DeriveInput);
    let struct_name = &input.ident;
    let state_trait = extract_state_trait(&input);

    // Add marker field to struct
    if let syn::Data::Struct(ref mut struct_data) = input.data {
        if let syn::Fields::Named(ref mut fields) = struct_data.fields {
            fields.named.push(
                syn::Field::parse_named
                    .parse2(quote! { marker: std::marker::PhantomData<S> })
                    .unwrap(),
            );
        }
    }

    let fields = match &input.data {
        syn::Data::Struct(s) => match &s.fields {
            syn::Fields::Named(fields) => {
                let field_names = fields
                    .named
                    .iter()
                    .filter(|f| f.ident.as_ref().map_or(false, |i| i != "marker"))
                    .map(|f| &f.ident);
                quote! {
                    #(#field_names: self.#field_names,)*
                    marker: std::marker::PhantomData,
                }
            }
            _ => panic!("Only named fields are supported"),
        },
        _ => panic!("Only structs are supported"),
    };

    // Constructor generation remains the same...
    let constructor = generate_constructor(&input, &state_trait);

    let transition_impl = quote! {
        impl<CurrentState: #state_trait> #struct_name<CurrentState> {
            pub fn into_context<NewState: #state_trait>(self) -> #struct_name<NewState>
            where NewState: #state_trait<Data = ()>
            {
                #struct_name {
                    #fields
                }
            }

            pub fn into_context_with<NewState: #state_trait>(self, data: NewState::Data) -> #struct_name<NewState> {
                #struct_name {
                    #fields
                }
            }
        }
    };

    let expanded = quote! {
        #input
        #transition_impl
        #constructor
    };

    TokenStream::from(expanded)
}

fn generate_constructor(input: &DeriveInput, state_trait: &syn::Ident) -> proc_macro2::TokenStream {
    let struct_name = &input.ident;

    let constructor_fields = match &input.data {
        syn::Data::Struct(s) => match &s.fields {
            syn::Fields::Named(fields) => {
                let param_names = fields
                    .named
                    .iter()
                    .filter(|f| f.ident.as_ref().map_or(false, |i| i != "marker"))
                    .map(|f| &f.ident);
                let param_types = fields
                    .named
                    .iter()
                    .filter(|f| f.ident.as_ref().map_or(false, |i| i != "marker"))
                    .map(|f| &f.ty);
                (
                    param_names.collect::<Vec<_>>(),
                    param_types.collect::<Vec<_>>(),
                )
            }
            _ => panic!("Only named fields are supported"),
        },
        _ => panic!("Only structs are supported"),
    };

    let (param_names, param_types) = constructor_fields;
    quote! {
        impl<S: #state_trait> #struct_name<S> {
            pub fn new(#(#param_names: #param_types),*) -> Self {
                Self {
                    #(#param_names,)*
                    marker: std::marker::PhantomData
                }
            }
        }
    }
}

fn extract_state_trait(input: &DeriveInput) -> syn::Ident {
    let generics = &input.generics;

    // Get the first type parameter
    let type_param = generics
        .type_params()
        .next()
        .expect("Struct must have a type parameter");

    // Get its bounds
    let bounds = &type_param.bounds;

    // Find the trait bound
    for bound in bounds {
        if let syn::TypeParamBound::Trait(trait_bound) = bound {
            // Get the last segment of the trait path (the trait name)
            if let Some(segment) = trait_bound.path.segments.last() {
                return segment.ident.clone();
            }
        }
    }

    panic!("Type parameter must have a trait bound")
}
//#[proc_macro_attribute]
//pub fn transition(_attr: TokenStream, item: TokenStream) -> TokenStream {
//    let input = parse_macro_input!(item as ItemImpl);
//
//    // Get all methods in the impl block
//    let updated_items = input.items.iter().map(|item| {
//        if let ImplItem::Fn(method) = item {
//            // Create new function with transformed body
//            let mut new_method = method.clone();
//
//            // Transform the function body to add .into_context()
//            if let syn::ReturnType::Type(_, ty) = &method.sig.output {
//                // Check if return type is Result<Context<_>>
//                if let Type::Path(type_path) = &**ty {
//                    if is_result_type(type_path) {
//                        transform_method_body(&mut new_method);
//                    }
//                }
//            }
//
//            ImplItem::Fn(new_method)
//        } else {
//            item.clone()
//        }
//    });
//
//    // Reconstruct the impl block with transformed methods
//    let mut new_impl = input.clone();
//    new_impl.items = updated_items.collect();
//
//    quote! {
//        #new_impl
//    }
//    .into()
//}
//
//fn is_result_type(type_path: &TypePath) -> bool {
//    type_path
//        .path
//        .segments
//        .iter()
//        .any(|segment| segment.ident == "Result")
//}
//
//fn transform_method_body(method: &mut ImplItemFn) {
//    let new_body = if let Some(stmt) = extract_return_expr(&method.block) {
//        if let Expr::Call(call_expr) = stmt {
//            if is_ok_call(call_expr) {
//                let inner_expr = &call_expr.args[0];
//                quote! {
//                    {
//                        Ok(#inner_expr.into_context())
//                    }
//                }
//            } else {
//                quote! { #stmt }
//            }
//        } else {
//            quote! { #stmt }
//        }
//    } else {
//        quote! { #method.block }
//    };
//
//    method.block = parse_quote! { #new_body };
//}
//
//fn extract_return_expr(block: &Block) -> Option<&Expr> {
//    if let Some(Stmt::Expr(expr, ..)) = block.stmts.last() {
//        Some(expr)
//    } else {
//        None
//    }
//}
//
//fn is_ok_call(expr: &ExprCall) -> bool {
//    if let Expr::Path(path) = &*expr.func {
//        path.path
//            .segments
//            .iter()
//            .any(|segment| segment.ident == "Ok")
//    } else {
//        false
//    }
//}
