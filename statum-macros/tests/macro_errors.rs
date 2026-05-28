#[test]
fn test_invalid_state_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_state_attr_args.rs");
    t.compile_fail("tests/ui/invalid_state_not_enum.rs");
    t.compile_fail("tests/ui/invalid_state_empty_enum.rs");
    t.compile_fail("tests/ui/invalid_state_cfg_variant.rs");
    t.compile_fail("tests/ui/invalid_state_cfg_payload_field.rs");
    t.compile_fail("tests/ui/invalid_state_named_field_payload_collision.rs");
    t.compile_fail("tests/ui/invalid_state_tuple_variant.rs");
    t.compile_fail("tests/ui/invalid_state_with_generics.rs");
    t.compile_fail("tests/ui/invalid_presentation_duplicate_key.rs");
    t.compile_fail("tests/ui/invalid_presentation_missing_parens.rs");
    t.compile_fail("tests/ui/invalid_presentation_unknown_key.rs");
}

#[cfg(feature = "introspection")]
#[test]
fn test_invalid_introspection_presentation_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_presentation_metadata_without_types.rs");
}

#[test]
fn test_invalid_machine_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_machine_not_struct.rs");
    t.compile_fail("tests/ui/invalid_machine_cfg_field.rs");
    t.compile_fail("tests/ui/invalid_machine_no_state_generic.rs");
    t.compile_fail("tests/ui/invalid_machine_wrong_generic.rs");
    t.compile_fail("tests/ui/invalid_machine_generic_not_first.rs");
    t.compile_fail("tests/ui/invalid_machine_private_field_access.rs");
    t.compile_fail("tests/ui/invalid_machine_missing_state_derive.rs");
    t.compile_fail("tests/ui/invalid_machine_plain_enum_missing_state_attr.rs");
    t.compile_fail("tests/ui/invalid_machine_declared_before_state.rs");
    t.compile_fail("tests/ui/invalid_machine_unknown_attr_key.rs");
    t.compile_fail("tests/ui/invalid_machine_builder_reserved_field_name.rs");
    t.compile_fail("tests/ui/invalid_machine_builder_duplicate_field.rs");
    t.compile_fail("tests/ui/invalid_machine_builder_duplicate_state_data.rs");
}

#[test]
fn test_invalid_transition_attribute_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_transition_attr_args.rs");
    t.compile_fail("tests/ui/invalid_transition_introspect_missing_return.rs");
    t.compile_fail("tests/ui/invalid_transition_introspect_unknown_key.rs");
}

#[test]
fn test_invalid_validators_attribute_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_validators_missing_machine_path.rs");
}

#[cfg(not(feature = "strict-introspection"))]
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
    t.compile_fail("tests/ui/invalid_transition_cfg_ambiguous_alias.rs");
    t.compile_fail("tests/ui/invalid_transition_custom_option_enum.rs");
    t.compile_fail("tests/ui/invalid_transition_custom_result_enum.rs");
    t.compile_fail("tests/ui/invalid_transition_custom_branch_same_name.rs");
    t.compile_fail("tests/ui/invalid_transition_custom_branch_enum.rs");
    t.compile_fail("tests/ui/invalid_transition_foreign_same_leaf_machine.rs");
    t.compile_fail("tests/ui/invalid_transition_macro_generated_alias.rs");
    t.compile_fail("tests/ui/invalid_transition_include_generated_alias.rs");
    t.compile_fail("tests/ui/invalid_transition_result_machine_in_error_branch.rs");
    t.compile_fail("tests/ui/invalid_transition_introspect_primary_branch_mismatch.rs");
    t.compile_fail("tests/ui/invalid_transition_introspect_override_non_machine_return.rs");
    t.compile_fail("tests/ui/invalid_transition_map_undeclared_edge.rs");
    t.compile_fail("tests/ui/invalid_transition_include_ambiguous_machine_name.rs");
    t.compile_fail("tests/ui/invalid_legacy_transition_helper_trait.rs");
}

#[cfg(all(not(feature = "strict-introspection"), not(feature = "introspection")))]
#[test]
fn test_default_introspection_surface_is_absent() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_default_introspection_surface.rs");
}

#[cfg(not(feature = "strict-introspection"))]
#[test]
fn test_invalid_validators_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_validators_missing_variant.rs");
    t.compile_fail("tests/ui/invalid_validators_wrong_return.rs");
    t.compile_fail("tests/ui/invalid_validators_alias_wrong_payload.rs");
    t.compile_fail("tests/ui/invalid_validators_wrong_signature.rs");
    t.compile_fail("tests/ui/invalid_validators_wrong_receiver.rs");
    t.compile_fail("tests/ui/invalid_validators_no_methods.rs");
    t.compile_fail("tests/ui/invalid_validators_no_methods_non_fn_items.rs");
    t.compile_fail("tests/ui/invalid_validators_unknown_state_method.rs");
    t.compile_fail("tests/ui/invalid_validators_unknown_machine.rs");
    t.compile_fail("tests/ui/invalid_validators_relative_path_alias.rs");
    t.compile_fail("tests/ui/invalid_validators_plain_struct_machine_name.rs");
    t.compile_fail("tests/ui/invalid_validators_parameter_name_collision.rs");
    t.compile_fail("tests/ui/invalid_validators_declared_before_machine.rs");
    t.compile_fail("tests/ui/invalid_rebuild_builder_duplicate_field.rs");
    t.compile_fail("tests/ui/invalid_rebuild_many_builder_duplicate_field.rs");
    t.compile_fail("tests/ui/invalid_legacy_superstate.rs");
    t.compile_fail("tests/ui/invalid_legacy_machine_builder.rs");
    t.compile_fail("tests/ui/invalid_legacy_machines_builder.rs");
    t.compile_fail("tests/ui/invalid_legacy_state_helper_traits.rs");
}

#[cfg(not(feature = "strict-introspection"))]
#[test]
fn test_valid_macro_usage() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/valid_state_unit_only.rs");
    t.pass("tests/ui/valid_state_with_data.rs");
    t.pass("tests/ui/valid_state_named_fields.rs");
    t.pass("tests/ui/valid_machine_no_fields.rs");
    t.pass("tests/ui/valid_machine_state_surface.rs");
    t.pass("tests/ui/valid_validators_sync.rs");
    t.pass("tests/ui/valid_validators_result_aliases.rs");
    t.pass("tests/ui/valid_validators_source_aliases.rs");
    t.pass("tests/ui/valid_validators_diagnostic_returns.rs");
    t.pass("tests/ui/valid_validators_generic_payload.rs");
    t.pass("tests/ui/valid_validators_async.rs");
    t.pass("tests/ui/valid_validators_hygienic_locals.rs");
    t.pass("tests/ui/valid_matrix.rs");
    t.pass("tests/ui/valid_same_names_different_modules.rs");
    t.pass("tests/ui/valid_transition_nested_wrappers.rs");
    t.pass("tests/ui/valid_transition_source_aliases.rs");
    t.pass("tests/ui/valid_transition_self_qualified_machine.rs");
    t.pass("tests/ui/valid_transition_crate_aliases.rs");
    t.pass("tests/ui/strict_valid_transition_introspect_return.rs");
    t.pass("tests/ui/valid_into_machines_by.rs");
    t.pass("tests/ui/valid_transition_map.rs");
    #[cfg(not(feature = "introspection"))]
    t.pass("tests/ui/valid_default_typestate_without_introspection_surface.rs");
    t.pass("tests/ui/valid_visibility_and_reconstruction.rs");
    t.pass("tests/ui/valid_multiple_machines_same_module.rs");
    t.pass("tests/ui/valid_machine_field_aliases.rs");
    t.pass("tests/ui/valid_machine_field_aliases_batch.rs");
    t.pass("tests/ui/valid_machine_field_aliases_local_validators.rs");
    t.pass("tests/ui/valid_machine_field_module_paths.rs");
    t.pass("tests/ui/valid_machine_field_aliases_renamed_import.rs");
    t.pass("tests/ui/valid_validators_relative_module_path.rs");
    t.pass("tests/ui/strict_valid_validators_explicit_machine_path.rs");
    t.pass("tests/ui/valid_cfg_hidden_duplicate_state_machine.rs");
    t.pass("tests/ui/valid_builder_usage.rs");
    t.pass("tests/ui/valid_helper_trait_visibility.rs");
    t.pass("tests/ui/valid_advanced_traits.rs");
}

#[cfg(all(not(feature = "strict-introspection"), feature = "introspection"))]
#[test]
fn test_valid_introspection_macro_usage() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/valid_machine_borrowed_data.rs");
    t.pass("tests/ui/valid_machine_extra_generics.rs");
    t.pass("tests/ui/valid_transition_branch.rs");
    t.pass("tests/ui/valid_machine_introspection.rs");
    t.pass("tests/ui/valid_machine_introspection_cfg_dedup.rs");
    t.pass("tests/ui/valid_presentation_sugar.rs");
    t.pass("tests/ui/valid_presentation_typed_metadata.rs");
    t.pass("tests/ui/workspace_member/crates/app/src/lib.rs");
}

#[cfg(feature = "strict-introspection")]
#[test]
fn test_invalid_transition_usage_strict() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/strict_invalid_transition_alias_requires_introspect.rs");
    t.compile_fail("tests/ui/strict_invalid_transition_result_machine_in_error_branch.rs");
    t.compile_fail("tests/ui/invalid_transition_introspect_primary_branch_mismatch.rs");
    t.compile_fail("tests/ui/invalid_transition_introspect_override_non_machine_return.rs");
}

#[cfg(feature = "strict-introspection")]
#[test]
fn test_invalid_validators_usage_strict() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/strict_invalid_validators_relative_path.rs");
}

#[cfg(feature = "strict-introspection")]
#[test]
fn test_valid_macro_usage_strict() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/strict_valid_transition_direct.rs");
    t.pass("tests/ui/strict_valid_transition_introspect_return.rs");
    t.pass("tests/ui/strict_valid_validators_explicit_machine_path.rs");
    t.pass("tests/ui/strict_valid_validators_self_path.rs");
    t.pass("tests/ui/strict_valid_validators_super_path.rs");
    t.pass("tests/ui/strict_valid_validators_external_layout.rs");
}
