use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{ToTokens, format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Path, PathArguments, Result, Token, Type, bracketed, parenthesized};

use crate::resolved_current_module_path;

mod kw {
    syn::custom_keyword!(journey);
    syn::custom_keyword!(label);
    syn::custom_keyword!(docs);
    syn::custom_keyword!(entry);
    syn::custom_keyword!(steps);
    syn::custom_keyword!(outcome);
}

pub fn parse_journeys(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as JourneysInput);
    if input.journeys.is_empty() {
        return syn::Error::new(
            Span::call_site(),
            "Error: `journeys!` requires at least one `journey <id> { ... }` declaration.",
        )
        .to_compile_error()
        .into();
    }

    let mut expanded = proc_macro2::TokenStream::new();
    for journey in input.journeys {
        let module_path = match resolved_current_module_path(journey.id.span(), "journeys!") {
            Ok(path) => path,
            Err(err) => return err,
        };
        match expand_journey(&journey, &module_path) {
            Ok(tokens) => expanded.extend(tokens),
            Err(err) => return err.to_compile_error().into(),
        }
    }

    expanded.into()
}

struct JourneysInput {
    journeys: Vec<JourneyDecl>,
}

impl Parse for JourneysInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut journeys = Vec::new();
        while !input.is_empty() {
            input.parse::<kw::journey>()?;
            let id = input.parse::<Ident>()?;
            let content;
            syn::braced!(content in input);
            journeys.push(JourneyDecl::parse_with_id(id, &content)?);
            if input.peek(Token![;]) {
                input.parse::<Token![;]>()?;
            }
        }
        Ok(Self { journeys })
    }
}

struct JourneyDecl {
    id: Ident,
    label: Option<LitStr>,
    docs: Option<LitStr>,
    entry: StepDecl,
    steps: Vec<StepDecl>,
    outcome: StepDecl,
}

impl JourneyDecl {
    fn parse_with_id(id: Ident, input: ParseStream<'_>) -> Result<Self> {
        let mut label = None;
        let mut docs = None;
        let mut entry = None;
        let mut steps = None;
        let mut outcome = None;

        while !input.is_empty() {
            if input.peek(kw::label) {
                input.parse::<kw::label>()?;
                input.parse::<Token![:]>()?;
                if label.is_some() {
                    return Err(syn::Error::new(input.span(), "duplicate `label` field"));
                }
                label = Some(input.parse::<LitStr>()?);
            } else if input.peek(kw::docs) {
                input.parse::<kw::docs>()?;
                input.parse::<Token![:]>()?;
                if docs.is_some() {
                    return Err(syn::Error::new(input.span(), "duplicate `docs` field"));
                }
                docs = Some(input.parse::<LitStr>()?);
            } else if input.peek(kw::entry) {
                input.parse::<kw::entry>()?;
                input.parse::<Token![:]>()?;
                if entry.is_some() {
                    return Err(syn::Error::new(input.span(), "duplicate `entry` field"));
                }
                entry = Some(input.parse::<StepDecl>()?);
            } else if input.peek(kw::steps) {
                input.parse::<kw::steps>()?;
                input.parse::<Token![:]>()?;
                if steps.is_some() {
                    return Err(syn::Error::new(input.span(), "duplicate `steps` field"));
                }
                let content;
                bracketed!(content in input);
                let parsed = content.parse_terminated(StepDecl::parse, Token![,])?;
                steps = Some(parsed.into_iter().collect::<Vec<_>>());
            } else if input.peek(kw::outcome) {
                input.parse::<kw::outcome>()?;
                input.parse::<Token![:]>()?;
                if outcome.is_some() {
                    return Err(syn::Error::new(input.span(), "duplicate `outcome` field"));
                }
                outcome = Some(input.parse::<StepDecl>()?);
            } else {
                return Err(input.error(
                    "expected one of `label`, `docs`, `entry`, `steps`, or `outcome`",
                ));
            }

            if input.peek(Token![;]) {
                input.parse::<Token![;]>()?;
            }
        }

        let entry = entry.ok_or_else(|| syn::Error::new(id.span(), "missing `entry` field"))?;
        let steps = steps.ok_or_else(|| syn::Error::new(id.span(), "missing `steps` field"))?;
        let outcome =
            outcome.ok_or_else(|| syn::Error::new(id.span(), "missing `outcome` field"))?;

        if steps.is_empty() {
            return Err(syn::Error::new(
                id.span(),
                "Error: journeys require at least one intermediate `steps` item.",
            ));
        }

        if matches!(entry, StepDecl::Bridge { .. }) {
            return Err(syn::Error::new(
                id.span(),
                "Error: `entry` cannot be `bridge!(...)`.\nFix: start a journey from `machine!(...)`, `state!(...)`, or `validator!(...)`.",
            ));
        }
        if matches!(outcome, StepDecl::Bridge { .. }) {
            return Err(syn::Error::new(
                id.span(),
                "Error: `outcome` cannot be `bridge!(...)`.\nFix: finish a journey at `machine!(...)` or `state!(...)`.",
            ));
        }

        if steps.windows(2).any(|window| {
            matches!(window[0], StepDecl::Bridge { .. }) && matches!(window[1], StepDecl::Bridge { .. })
        }) {
            return Err(syn::Error::new(
                id.span(),
                "Error: journeys do not allow adjacent `bridge!(...)` steps.\nFix: place a `machine!(...)` or `state!(...)` step between declared bridges.",
            ));
        }

        Ok(Self {
            id,
            label,
            docs,
            entry,
            steps,
            outcome,
        })
    }
}

enum StepDecl {
    Machine {
        path: Path,
    },
    State {
        machine_path: Path,
        state: Ident,
    },
    Validator {
        source_type: Type,
        machine_path: Path,
    },
    Bridge {
        bridge_type: Type,
    },
}

impl Parse for StepDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let macro_name = input.parse::<Ident>()?;
        input.parse::<Token![!]>()?;
        let content;
        parenthesized!(content in input);
        match macro_name.to_string().as_str() {
            "machine" => {
                let path = content.parse::<Path>()?;
                Ok(Self::Machine { path })
            }
            "state" => {
                let machine_path = content.parse::<Path>()?;
                content.parse::<Token![,]>()?;
                let state = content.parse::<Ident>()?;
                Ok(Self::State {
                    machine_path,
                    state,
                })
            }
            "validator" => {
                let source_type = content.parse::<Type>()?;
                content.parse::<Token![=>]>()?;
                let machine_path = content.parse::<Path>()?;
                Ok(Self::Validator {
                    source_type,
                    machine_path,
                })
            }
            "bridge" => {
                let bridge_type = content.parse::<Type>()?;
                Ok(Self::Bridge { bridge_type })
            }
            _ => Err(syn::Error::new(
                macro_name.span(),
                "Error: journey steps must use `machine!(...)`, `state!(...)`, `validator!(...)`, or `bridge!(...)`.",
            )),
        }
    }
}

fn expand_journey(journey: &JourneyDecl, source_module_path: &str) -> Result<proc_macro2::TokenStream> {
    let module_path_lit = LitStr::new(source_module_path, Span::call_site());
    let id_lit = LitStr::new(&journey.id.to_string(), journey.id.span());
    let label_tokens = journey
        .label
        .as_ref()
        .map(|label| quote! { Some(#label) })
        .unwrap_or_else(|| quote! { None });
    let docs_tokens = journey
        .docs
        .as_ref()
        .map(|docs| quote! { Some(#docs) })
        .unwrap_or_else(|| quote! { None });

    let mut helpers = Vec::new();
    let entry = expand_step(&journey.entry, source_module_path, &journey.id, 0, &mut helpers)?;
    let steps = journey
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| expand_step(step, source_module_path, &journey.id, index + 1, &mut helpers))
        .collect::<Result<Vec<_>>>()?;
    let outcome = expand_step(
        &journey.outcome,
        source_module_path,
        &journey.id,
        journey.steps.len() + 1,
        &mut helpers,
    )?;

    let line_number = journey.id.span().start().line;
    let registration_ident = linked_journey_registration_ident(&journey.id.to_string(), source_module_path, line_number);
    let steps_ident = linked_journey_steps_ident(&journey.id.to_string(), source_module_path, line_number);

    Ok(quote! {
        #(#helpers)*

        #[doc(hidden)]
        static #steps_ident: &[statum::__private::LinkedJourneyStepDescriptor] = &[
            #(#steps),*
        ];

        #[doc(hidden)]
        #[statum::__private::linkme::distributed_slice(statum::__private::__STATUM_LINKED_JOURNEYS)]
        #[linkme(crate = statum::__private::linkme)]
        static #registration_ident: statum::__private::LinkedJourneyDescriptor =
            statum::__private::LinkedJourneyDescriptor {
                module_path: #module_path_lit,
                id: #id_lit,
                label: #label_tokens,
                docs: #docs_tokens,
                entry: #entry,
                steps: #steps_ident,
                outcome: #outcome,
            };
    })
}

fn expand_step(
    step: &StepDecl,
    source_module_path: &str,
    journey_id: &Ident,
    step_index: usize,
    helpers: &mut Vec<proc_macro2::TokenStream>,
) -> Result<proc_macro2::TokenStream> {
    match step {
        StepDecl::Machine { path } => {
            let machine_path = resolve_explicit_path(
                path,
                source_module_path,
                "machine",
                "machine!(crate::workflow::Machine)",
            )?;
            let machine_path_tokens = string_slice_tokens(&machine_path);
            Ok(quote! {
                statum::__private::LinkedJourneyStepDescriptor::Machine {
                    machine_path: &[#(#machine_path_tokens),*],
                }
            })
        }
        StepDecl::State { machine_path, state } => {
            let machine_path = resolve_explicit_path(
                machine_path,
                source_module_path,
                "state machine",
                "state!(crate::workflow::Machine, Done)",
            )?;
            let machine_path_tokens = string_slice_tokens(&machine_path);
            let state_lit = LitStr::new(&state.to_string(), state.span());
            Ok(quote! {
                statum::__private::LinkedJourneyStepDescriptor::State {
                    machine_path: &[#(#machine_path_tokens),*],
                    state: #state_lit,
                }
            })
        }
        StepDecl::Validator {
            source_type,
            machine_path,
        } => {
            let machine_path = resolve_explicit_path(
                machine_path,
                source_module_path,
                "validator machine",
                "validator!(crate::Row => crate::workflow::Machine)",
            )?;
            let machine_path_tokens = string_slice_tokens(&machine_path);
            let source_type_display = LitStr::new(
                &source_type.to_token_stream().to_string(),
                Span::call_site(),
            );
            let helper_ident =
                linked_journey_source_type_name_ident(journey_id, source_module_path, step_index);
            helpers.push(quote! {
                #[doc(hidden)]
                fn #helper_ident() -> &'static str {
                    ::core::any::type_name::<#source_type>()
                }
            });
            Ok(quote! {
                statum::__private::LinkedJourneyStepDescriptor::Validator {
                    source_type_display: #source_type_display,
                    resolved_source_type_name: #helper_ident,
                    machine_path: &[#(#machine_path_tokens),*],
                }
            })
        }
        StepDecl::Bridge { bridge_type } => {
            let type_display = LitStr::new(
                &bridge_type.to_token_stream().to_string(),
                Span::call_site(),
            );
            let helper_ident =
                linked_journey_bridge_type_name_ident(journey_id, source_module_path, step_index);
            helpers.push(quote! {
                #[doc(hidden)]
                fn #helper_ident() -> &'static str {
                    ::core::any::type_name::<#bridge_type>()
                }
            });
            Ok(quote! {
                statum::__private::LinkedJourneyStepDescriptor::Bridge {
                    type_display: #type_display,
                    resolved_type_name: #helper_ident,
                }
            })
        }
    }
}

fn resolve_explicit_path(
    path: &Path,
    source_module_path: &str,
    role: &str,
    example: &str,
) -> Result<Vec<String>> {
    let raw_segments = path
        .segments
        .iter()
        .map(|segment| {
            if !matches!(segment.arguments, PathArguments::None) {
                return None;
            }
            Some(segment.ident.to_string())
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            syn::Error::new_spanned(
                path,
                format!(
                    "Error: `{role}` journey references do not support generic arguments here.\nFix: use an explicit path like `{example}`."
                ),
            )
        })?;

    if raw_segments.is_empty() {
        return Err(syn::Error::new_spanned(
            path,
            format!(
                "Error: `{role}` journey references require an explicit path like `{example}`."
            ),
        ));
    }

    if path.leading_colon.is_some() {
        return Ok(raw_segments);
    }

    let module_segments = split_module_path(source_module_path);
    let mut resolved = Vec::new();
    let mut index = 0usize;
    match raw_segments.first().map(String::as_str) {
        Some("crate") => {
            let crate_root = module_segments.first().ok_or_else(|| {
                syn::Error::new_spanned(path, "internal error: missing crate root for journey path")
            })?;
            if raw_segments.get(1) != Some(crate_root) {
                resolved.push(crate_root.clone());
            }
            index = 1;
        }
        Some("self") => {
            resolved.extend(module_segments);
            index = 1;
        }
        Some("super") => {
            resolved.extend(module_segments);
            while matches!(raw_segments.get(index).map(String::as_str), Some("super")) {
                if resolved.len() <= 1 {
                    return Err(syn::Error::new_spanned(
                        path,
                        format!(
                            "Error: `{role}` journey path cannot walk above the crate root.\nFix: use an explicit `crate::`, `self::`, `super::`, or absolute path like `{example}`."
                        ),
                    ));
                }
                resolved.pop();
                index += 1;
            }
        }
        _ => {
            return Err(syn::Error::new_spanned(
                path,
                format!(
                    "Error: `{role}` journey references must use an explicit `crate::`, `self::`, `super::`, or absolute path like `{example}`."
                ),
            ));
        }
    }

    resolved.extend(raw_segments.into_iter().skip(index));
    Ok(resolved)
}

fn split_module_path(module_path: &str) -> Vec<String> {
    module_path
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn string_slice_tokens(values: &[String]) -> Vec<LitStr> {
    values
        .iter()
        .map(|value| LitStr::new(value, Span::call_site()))
        .collect()
}

fn linked_journey_registration_ident(id: &str, module_path: &str, line_number: usize) -> Ident {
    format_ident!(
        "__STATUM_LINKED_JOURNEY_{:016X}",
        stable_hash(&format!("{module_path}::{id}::{line_number}::journey"))
    )
}

fn linked_journey_steps_ident(id: &str, module_path: &str, line_number: usize) -> Ident {
    format_ident!(
        "__STATUM_LINKED_JOURNEY_STEPS_{:016X}",
        stable_hash(&format!("{module_path}::{id}::{line_number}::steps"))
    )
}

fn linked_journey_source_type_name_ident(
    id: &Ident,
    module_path: &str,
    step_index: usize,
) -> Ident {
    format_ident!(
        "__statum_journey_source_type_name_{:016x}",
        stable_hash(&format!(
            "{module_path}::{}::{step_index}::source_type_name",
            id
        ))
    )
}

fn linked_journey_bridge_type_name_ident(
    id: &Ident,
    module_path: &str,
    step_index: usize,
) -> Ident {
    format_ident!(
        "__statum_journey_bridge_type_name_{:016x}",
        stable_hash(&format!(
            "{module_path}::{}::{step_index}::bridge_type_name",
            id
        ))
    )
}

fn stable_hash(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
