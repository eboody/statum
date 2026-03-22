#[test]
fn test_invalid_state_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_state_not_enum.rs");
    t.compile_fail("tests/ui/invalid_state_empty_enum.rs");
    t.compile_fail("tests/ui/invalid_state_struct_variant.rs");
    t.compile_fail("tests/ui/invalid_state_tuple_variant.rs");
    t.compile_fail("tests/ui/invalid_state_with_generics.rs");
}

#[test]
fn test_invalid_machine_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_machine_not_struct.rs");
    t.compile_fail("tests/ui/invalid_machine_no_state_generic.rs");
    t.compile_fail("tests/ui/invalid_machine_wrong_generic.rs");
    t.compile_fail("tests/ui/invalid_machine_generic_not_first.rs");
    t.compile_fail("tests/ui/invalid_machine_multiple_generics.rs");
    t.compile_fail("tests/ui/invalid_machine_private_field_access.rs");
    t.compile_fail("tests/ui/invalid_machine_missing_state_derive.rs");
    t.compile_fail("tests/ui/invalid_machine_plain_enum_missing_state_attr.rs");
}

#[test]
fn test_invalid_transition_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_transition_no_methods.rs");
    t.compile_fail("tests/ui/invalid_transition_not_method.rs");
    t.compile_fail("tests/ui/invalid_transition_wrong_return.rs");
    t.compile_fail("tests/ui/invalid_transition_conditional.rs");
    t.compile_fail("tests/ui/invalid_transition_unknown_machine.rs");
    t.compile_fail("tests/ui/invalid_transition_plain_struct_machine_name.rs");
    t.compile_fail("tests/ui/invalid_transition_unknown_source_state.rs");
    t.compile_fail("tests/ui/invalid_transition_unknown_return_state.rs");
    t.compile_fail("tests/ui/invalid_transition_unknown_secondary_return_state.rs");
    t.compile_fail("tests/ui/invalid_transition_map_undeclared_edge.rs");
    t.compile_fail("tests/ui/invalid_legacy_transition_helper_trait.rs");
}

#[test]
fn test_invalid_validators_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_validators_missing_variant.rs");
    t.compile_fail("tests/ui/invalid_validators_wrong_return.rs");
    t.compile_fail("tests/ui/invalid_validators_alias_wrong_payload.rs");
    t.compile_fail("tests/ui/invalid_validators_wrong_signature.rs");
    t.compile_fail("tests/ui/invalid_validators_wrong_receiver.rs");
    t.compile_fail("tests/ui/invalid_validators_no_methods.rs");
    t.compile_fail("tests/ui/invalid_validators_unknown_state_method.rs");
    t.compile_fail("tests/ui/invalid_validators_unknown_machine.rs");
    t.compile_fail("tests/ui/invalid_validators_plain_struct_machine_name.rs");
    t.compile_fail("tests/ui/invalid_validators_parameter_name_collision.rs");
    t.compile_fail("tests/ui/invalid_legacy_superstate.rs");
    t.compile_fail("tests/ui/invalid_legacy_machine_builder.rs");
    t.compile_fail("tests/ui/invalid_legacy_machines_builder.rs");
    t.compile_fail("tests/ui/invalid_legacy_state_helper_traits.rs");
}

#[test]
fn test_valid_macro_usage() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/valid_state_unit_only.rs");
    t.pass("tests/ui/valid_state_with_data.rs");
    t.pass("tests/ui/valid_machine_no_fields.rs");
    t.pass("tests/ui/valid_machine_state_surface.rs");
    t.pass("tests/ui/valid_validators_sync.rs");
    t.pass("tests/ui/valid_validators_result_aliases.rs");
    t.pass("tests/ui/valid_validators_generic_payload.rs");
    t.pass("tests/ui/valid_validators_async.rs");
    t.pass("tests/ui/valid_matrix.rs");
    t.pass("tests/ui/valid_same_names_different_modules.rs");
    t.pass("tests/ui/valid_transition_nested_wrappers.rs");
    t.pass("tests/ui/valid_into_machines_by.rs");
    t.pass("tests/ui/valid_transition_map.rs");
    t.pass("tests/ui/valid_machine_introspection.rs");
    t.pass("tests/ui/valid_visibility_and_reconstruction.rs");
    t.pass("tests/ui/valid_multiple_machines_same_module.rs");
    t.pass("tests/ui/valid_machine_field_aliases.rs");
    t.pass("tests/ui/valid_machine_field_aliases_batch.rs");
    t.pass("tests/ui/valid_machine_field_aliases_local_validators.rs");
    t.pass("tests/ui/valid_machine_field_module_paths.rs");
    t.pass("tests/ui/valid_machine_field_aliases_renamed_import.rs");
    t.pass("tests/ui/valid_builder_overwrite.rs");
    t.pass("tests/ui/valid_helper_trait_visibility.rs");
    t.pass("tests/ui/valid_advanced_traits.rs");
}
