#![feature(proc_macro_span)]
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

use proc_macro::{Span, TokenStream};
use syn::{parse_macro_input, ItemEnum, ItemImpl, ItemStruct};

use module_path_extractor::get_pseudo_module_path;

#[proc_macro_attribute]
pub fn state(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let module_path = get_pseudo_module_path();
    println!("\nmodule_path:\n{:#?}", module_path);
    let input = parse_macro_input!(item as ItemEnum);

    // Validate the enum before proceeding
    if let Some(error) = validate_state_enum(&input) {
        return error.into();
    }

    let enum_info = EnumInfo::from_item_enum(&input).expect("Failed to parse EnumInfo");

    // Store metadata in `state_enum_map`
    store_state_enum(&enum_info);

    // Generate structs and implementations dynamically
    let expanded = generate_state_impls(&enum_info.module_path);

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn machine(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);

    let machine_info = MachineInfo::from_item_struct(&input).expect("Failed to parse MachineInfo");

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
    let tr_impl = parse_transition_impl(&input);

    let module_path = get_pseudo_module_path();

    let machine_map = get_machine_map().read().unwrap();
    let state_enum_map = read_state_enum_map();
    let state_enum_info = state_enum_map
        .get(&module_path.clone().into())
        .expect("State enum not found in proc macro transition");

    if let Some(err) =
        validate_machine_and_state(&tr_impl, &module_path, &machine_map, &state_enum_map)
    {
        return err.into();
    }

    // Retrieve references to the actual MachineInfo / EnumInfo if you need them
    // If you need them for codegen, you can do something like:
    let machine_info = machine_map
        .get(&module_path.clone().into())
        .expect("Machine not found, even though we validated above");

    if let Some(err) =
        validate_transition_functions(&tr_impl.functions, machine_info, state_enum_info)
    {
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
    let module_path = get_pseudo_module_path();
    parse_validators(attr, item, &module_path)
}
