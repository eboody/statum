mod emission;
mod generics;
mod introspection;
mod metadata;
mod registry;
mod validation;

pub use emission::generate_machine_impls;
pub(crate) use generics::{
    builder_generics, extra_generics, extra_type_arguments_tokens, generic_argument_tokens,
    machine_type_with_state,
};
pub(crate) use introspection::{
    linked_transition_slice_ident, to_shouty_snake_identifier,
    transition_presentation_slice_ident, transition_slice_ident,
};
pub use metadata::{MachineInfo, MachinePath};
pub use registry::{
    LoadedMachineLookupFailure, format_loaded_machine_candidates, lookup_loaded_machine_in_module,
    same_named_loaded_machines_elsewhere, store_machine_struct,
};
pub use validation::{invalid_machine_target_error, validate_machine_struct};
