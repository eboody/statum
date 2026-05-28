//! Transition macro pipeline: parse impls, resolve return-shape contracts, then emit code.

mod contract;
mod diagnostics;
mod emit;
mod parse;
mod pipeline;
mod resolve;
mod validation;

use crate::contracts::TransitionContract;

pub(crate) struct ValidatedTransitionMethod {
    pub(crate) function: parse::TransitionFn,
    pub(crate) contract: TransitionContract,
}

pub fn expand_transition(input: syn::ItemImpl) -> proc_macro2::TokenStream {
    pipeline::expand_transition(input)
}
