mod emission;
mod metadata;
mod registry;
mod validation;

pub(crate) use emission::transition_support_module_ident;
pub use emission::generate_machine_impls;
pub use metadata::{MachineInfo, MachinePath};
pub use registry::{ensure_machine_loaded_by_name, store_machine_struct};
pub use validation::{invalid_machine_target_error, validate_machine_struct};
