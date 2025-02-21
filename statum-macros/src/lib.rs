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

use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemEnum, ItemImpl, ItemStruct};

#[proc_macro_attribute]
pub fn state(_attr: TokenStream, item: TokenStream) -> TokenStream {
    println!("[state] Starting macro execution...");

    let input = parse_macro_input!(item as ItemEnum);

    // Validate the enum before proceeding
    if let Some(error) = validate_state_enum(&input) {
        return error.into();
    }

    let enum_info = EnumInfo::from_item_enum(&input).expect("Failed to parse EnumInfo");

    println!("[state] Parsed Enum: {}", enum_info.name);

    // Store metadata in `state_enum_map`
    store_state_enum(&enum_info);

    // Generate structs and implementations dynamically
    let expanded = generate_state_impls(&enum_info.file_path);

    println!("[state] Macro execution completed.");
    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn machine(_attr: TokenStream, item: TokenStream) -> TokenStream {
    println!("[machine] Starting macro execution...");

    let input = parse_macro_input!(item as ItemStruct);

    let machine_info = MachineInfo::from_item_struct(&input).expect("Failed to parse MachineInfo");

    // Validate the struct before proceeding
    if let Some(error) = validate_machine_struct(&input, &machine_info) {
        return error.into();
    }

    println!("[machine] Parsed Struct: {}", machine_info.name);

    // Store metadata in `machine_map`
    store_machine_struct(&machine_info);

    // Generate any required structs or implementations dynamically
    let expanded = generate_machine_impls(&machine_info);

    println!("[machine] Macro execution completed.");
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

    // -- Step 2: Perform validations
    let file_path = std::env::current_dir()
        .expect("Failed to get current directory.")
        .to_string_lossy()
        .to_string();

    let machine_map = get_machine_map().read().unwrap();
    let state_enum_map = read_state_enum_map();

    if let Some(err) =
        validate_machine_and_state(&tr_impl, &file_path, &machine_map, &state_enum_map)
    {
        return err.into();
    }
    if let Some(err) = validate_transition_functions(&tr_impl.functions) {
        return err.into();
    }

    // Retrieve references to the actual MachineInfo / EnumInfo if you need them
    // If you need them for codegen, you can do something like:
    let machine_info = machine_map
        .get(&file_path.clone().into())
        .expect("Machine not found, even though we validated above");

    // -- Step 3: Generate new code
    let expanded = generate_transition_impl(&tr_impl, machine_info, &file_path);

    // Combine expanded code with the original `impl` if needed
    // or simply return the expanded code
    expanded.into()
}

#[proc_macro_attribute]
pub fn validators(attr: TokenStream, item: TokenStream) -> TokenStream {
    parse_validators(attr, item)
}
