mod emission;
mod introspection;
mod metadata;
mod registry;
mod validation;

pub(crate) use emission::transition_support_module_ident;
pub use emission::generate_machine_impls;
pub(crate) use introspection::{to_shouty_snake_identifier, transition_slice_ident};
pub use metadata::{MachineInfo, MachinePath};
pub use registry::{
    LoadedMachineLookupFailure, format_loaded_machine_candidates,
    lookup_loaded_machine_in_module, lookup_unique_loaded_machine_by_name, store_machine_struct,
};
pub use validation::{invalid_machine_target_error, validate_machine_struct};
