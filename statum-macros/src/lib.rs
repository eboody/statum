moddef::moddef!(
    flat (pub) mod {
    },
    flat (pub(crate)) mod {
        state,
        machine,
        transition,
        validators
    }
);

use crate::{
    MachinePath, StateModulePath, ensure_machine_loaded_by_name, ensure_state_enum_loaded,
};
use macro_registry::callsite::current_module_path;
use proc_macro::TokenStream;
use syn::{Item, ItemImpl, parse_macro_input};

#[proc_macro_attribute]
pub fn state(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as Item);
    let input = match input {
        Item::Enum(item_enum) => item_enum,
        other => return invalid_state_target_error(&other).into(),
    };

    // Validate the enum before proceeding
    if let Some(error) = validate_state_enum(&input) {
        return error.into();
    }

    let enum_info = match EnumInfo::from_item_enum(&input) {
        Ok(info) => info,
        Err(err) => return err.to_compile_error().into(),
    };

    // Store metadata in `state_enum_map`
    store_state_enum(&enum_info);

    // Generate structs and implementations dynamically
    let expanded = generate_state_impls(&enum_info.module_path);

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn machine(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as Item);
    let input = match input {
        Item::Struct(item_struct) => item_struct,
        other => return invalid_machine_target_error(&other).into(),
    };

    let machine_info = MachineInfo::from_item_struct(&input);

    // Validate the struct before proceeding
    if let Some(error) = validate_machine_struct(&input, &machine_info) {
        return error.into();
    }

    // Store metadata in `machine_map`
    store_machine_struct(&machine_info);

    // Generate any required structs or implementations dynamically
    let expanded = generate_machine_impls(&machine_info);

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn transition(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as ItemImpl);

    // -- Step 1: Parse
    let tr_impl = match parse_transition_impl(&input) {
        Ok(parsed) => parsed,
        Err(err) => return err.into(),
    };

    let module_path = current_module_path();

    let state_path: StateModulePath = module_path.clone().into();
    let machine_path: MachinePath = module_path.clone().into();
    let _ = ensure_state_enum_loaded(&state_path);
    let machine_info_owned = ensure_machine_loaded_by_name(&machine_path, &tr_impl.machine_name);
    let machine_info = match machine_info_owned.as_ref() {
        Some(info) => info,
        None => {
            return missing_transition_machine_error(
                &tr_impl.machine_name,
                &module_path,
                tr_impl.machine_span,
            )
            .into();
        }
    };

    if let Some(err) = validate_transition_functions(&tr_impl, machine_info) {
        return err.into();
    }

    // -- Step 3: Generate new code
    let expanded = generate_transition_impl(&input, &tr_impl, machine_info, &module_path);

    // Combine expanded code with the original `impl` if needed
    // or simply return the expanded code
    expanded.into()
}

#[proc_macro_attribute]
pub fn validators(attr: TokenStream, item: TokenStream) -> TokenStream {
    let module_path = current_module_path();
    parse_validators(attr, item, &module_path)
}
