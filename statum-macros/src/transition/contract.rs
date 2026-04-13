use proc_macro2::TokenStream;

use crate::contracts::TransitionContract;
use crate::diagnostics::compact_display;

use super::parse::TransitionFn;

pub(super) fn build_transition_contract(
    func: &TransitionFn,
    target_type: &syn::Type,
) -> Result<TransitionContract, TokenStream> {
    Ok(TransitionContract {
        machine_name: func.machine_name.clone(),
        source_state_name: func.source_state.clone(),
        next_states: func.return_states(target_type)?,
        strict_introspection: crate::strict_introspection_enabled() || func.introspection.is_some(),
        written_return_type: func.return_type.as_ref().map(compact_display),
    })
}
