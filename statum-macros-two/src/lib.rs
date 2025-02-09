moddef::moddef!(
    flat (pub) mod {
    },
    flat (pub(crate)) mod {
        state,
        machine
    }
);

use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemEnum, ItemStruct};

#[proc_macro_attribute]
pub fn state(_attr: TokenStream, item: TokenStream) -> TokenStream {
    println!("[state] Starting macro execution...");

    let input = parse_macro_input!(item as ItemEnum);

    // Validate the enum before proceeding
    if let Some(error) = validate_state_enum(&input) {
        return error.into();
    }

    let enum_info = EnumInfo::from_item_enum(&input);

    println!("[state] Parsed Enum: {}", enum_info.name.0);

    // Store metadata in `state_enum_map`
    store_state_enum(&enum_info);

    // Generate structs and implementations dynamically
    let expanded = generate_state_impls(&enum_info.name);

    println!("[state] Macro execution completed.");
    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn machine(_attr: TokenStream, item: TokenStream) -> TokenStream {
    println!("[machine] Starting macro execution...");

    let input = parse_macro_input!(item as ItemStruct);

    // Validate the struct before proceeding
    if let Some(error) = validate_machine_struct(&input) {
        return error.into();
    }

    let machine_info = MachineInfo::from_item_struct(&input);
    let generics = input.generics;

    println!("[machine] Parsed Struct: {}", machine_info.name.0);

    // Store metadata in `machine_map`
    store_machine_struct(&machine_info);

    // Generate any required structs or implementations dynamically
    let expanded = generate_machine_impls(&machine_info.name, generics);

    println!("[machine] Macro execution completed.");
    TokenStream::from(expanded)
}
