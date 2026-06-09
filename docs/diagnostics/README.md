# Diagnostics Guide

This guide is assembled from the current first-party diagnostic test surface:
the `statum-macros/tests/macro_errors.rs` compile-fail registrations and their
matching committed `statum-macros/tests/ui/*.stderr` fixtures.

Authority surface: committed trybuild compile-fail fixtures. Observation point:
source fixtures and expected compiler output, not expanded macros or runtime
values. Unsupported cases are tracked as compiler-fallback placeholders instead
of being described as polished first-party diagnostics.

Each first-party diagnostic page includes:

- the broken source fixture,
- the expected compiler output,
- a corrected shape that demonstrates the intended fix,
- the diagnostic's `Found`, `Expected`, and `Fix` guidance where present.

Pages marked as placeholders intentionally rely on native Rust errors today.
They remain in this guide so maintainers can see every known diagnostic or
regression fixture in one place.

Known fixture count: 76. First-party diagnostic pages: 62. Compiler-fallback placeholders: 14.

## State
- [invalid_builder_missing_state_data](invalid_builder_missing_state_data.md) — placeholder
- [invalid_machine_builder_duplicate_state_data](invalid_machine_builder_duplicate_state_data.md) — placeholder
- [invalid_machine_declared_before_state](invalid_machine_declared_before_state.md) — diagnostic
- [invalid_machine_missing_state_derive](invalid_machine_missing_state_derive.md) — diagnostic
- [invalid_machine_no_state_generic](invalid_machine_no_state_generic.md) — diagnostic
- [invalid_machine_plain_enum_missing_state_attr](invalid_machine_plain_enum_missing_state_attr.md) — diagnostic
- [invalid_state_attr_args](invalid_state_attr_args.md) — diagnostic
- [invalid_state_cfg_payload_field](invalid_state_cfg_payload_field.md) — diagnostic
- [invalid_state_cfg_variant](invalid_state_cfg_variant.md) — diagnostic
- [invalid_state_empty_enum](invalid_state_empty_enum.md) — diagnostic
- [invalid_state_named_field_payload_collision](invalid_state_named_field_payload_collision.md) — placeholder
- [invalid_state_not_enum](invalid_state_not_enum.md) — diagnostic
- [invalid_state_tuple_variant](invalid_state_tuple_variant.md) — diagnostic
- [invalid_state_with_generics](invalid_state_with_generics.md) — diagnostic

## Machine And Builder
- [invalid_builder_missing_machine_field](invalid_builder_missing_machine_field.md) — placeholder
- [invalid_machine_builder_duplicate_field](invalid_machine_builder_duplicate_field.md) — placeholder
- [invalid_machine_builder_reserved_field_name](invalid_machine_builder_reserved_field_name.md) — diagnostic
- [invalid_machine_cfg_field](invalid_machine_cfg_field.md) — diagnostic
- [invalid_machine_generic_not_first](invalid_machine_generic_not_first.md) — diagnostic
- [invalid_machine_not_struct](invalid_machine_not_struct.md) — diagnostic
- [invalid_machine_private_field_access](invalid_machine_private_field_access.md) — placeholder
- [invalid_machine_unknown_attr_key](invalid_machine_unknown_attr_key.md) — diagnostic
- [invalid_machine_wrong_generic](invalid_machine_wrong_generic.md) — diagnostic
- [invalid_rebuild_builder_duplicate_field](invalid_rebuild_builder_duplicate_field.md) — placeholder
- [invalid_rebuild_many_builder_duplicate_field](invalid_rebuild_many_builder_duplicate_field.md) — placeholder

## Presentation
- [invalid_presentation_duplicate_key](invalid_presentation_duplicate_key.md) — diagnostic
- [invalid_presentation_metadata_without_types](invalid_presentation_metadata_without_types.md) — diagnostic
- [invalid_presentation_missing_parens](invalid_presentation_missing_parens.md) — diagnostic
- [invalid_presentation_unknown_key](invalid_presentation_unknown_key.md) — diagnostic

## Transition
- [invalid_transition_attr_args](invalid_transition_attr_args.md) — diagnostic
- [invalid_transition_cfg_ambiguous_alias](invalid_transition_cfg_ambiguous_alias.md) — diagnostic
- [invalid_transition_conditional](invalid_transition_conditional.md) — diagnostic
- [invalid_transition_custom_branch_enum](invalid_transition_custom_branch_enum.md) — diagnostic
- [invalid_transition_custom_branch_same_name](invalid_transition_custom_branch_same_name.md) — diagnostic
- [invalid_transition_custom_option_enum](invalid_transition_custom_option_enum.md) — diagnostic
- [invalid_transition_custom_result_enum](invalid_transition_custom_result_enum.md) — diagnostic
- [invalid_transition_foreign_same_leaf_machine](invalid_transition_foreign_same_leaf_machine.md) — diagnostic
- [invalid_transition_include_ambiguous_machine_name](invalid_transition_include_ambiguous_machine_name.md) — diagnostic
- [invalid_transition_include_generated_alias](invalid_transition_include_generated_alias.md) — diagnostic
- [invalid_transition_introspect_missing_return](invalid_transition_introspect_missing_return.md) — diagnostic
- [invalid_transition_introspect_override_non_machine_return](invalid_transition_introspect_override_non_machine_return.md) — diagnostic
- [invalid_transition_introspect_primary_branch_mismatch](invalid_transition_introspect_primary_branch_mismatch.md) — diagnostic
- [invalid_transition_introspect_unknown_key](invalid_transition_introspect_unknown_key.md) — diagnostic
- [invalid_transition_macro_generated_alias](invalid_transition_macro_generated_alias.md) — diagnostic
- [invalid_transition_map_undeclared_edge](invalid_transition_map_undeclared_edge.md) — placeholder
- [invalid_transition_no_methods](invalid_transition_no_methods.md) — diagnostic
- [invalid_transition_not_method](invalid_transition_not_method.md) — diagnostic
- [invalid_transition_plain_struct_machine_name](invalid_transition_plain_struct_machine_name.md) — diagnostic
- [invalid_transition_result_machine_in_error_branch](invalid_transition_result_machine_in_error_branch.md) — diagnostic
- [invalid_transition_unknown_machine](invalid_transition_unknown_machine.md) — diagnostic
- [invalid_transition_unknown_return_state](invalid_transition_unknown_return_state.md) — diagnostic
- [invalid_transition_unknown_secondary_return_state](invalid_transition_unknown_secondary_return_state.md) — diagnostic
- [invalid_transition_unknown_source_state](invalid_transition_unknown_source_state.md) — diagnostic
- [invalid_transition_wrong_return](invalid_transition_wrong_return.md) — diagnostic
- [strict_invalid_transition_alias_requires_introspect](strict_invalid_transition_alias_requires_introspect.md) — diagnostic
- [strict_invalid_transition_result_machine_in_error_branch](strict_invalid_transition_result_machine_in_error_branch.md) — diagnostic

## Validators
- [invalid_validators_alias_wrong_payload](invalid_validators_alias_wrong_payload.md) — diagnostic
- [invalid_validators_declared_before_machine](invalid_validators_declared_before_machine.md) — diagnostic
- [invalid_validators_missing_machine_path](invalid_validators_missing_machine_path.md) — diagnostic
- [invalid_validators_missing_variant](invalid_validators_missing_variant.md) — diagnostic
- [invalid_validators_no_methods](invalid_validators_no_methods.md) — diagnostic
- [invalid_validators_no_methods_non_fn_items](invalid_validators_no_methods_non_fn_items.md) — diagnostic
- [invalid_validators_parameter_name_collision](invalid_validators_parameter_name_collision.md) — diagnostic
- [invalid_validators_plain_struct_machine_name](invalid_validators_plain_struct_machine_name.md) — diagnostic
- [invalid_validators_relative_path_alias](invalid_validators_relative_path_alias.md) — diagnostic
- [invalid_validators_unknown_machine](invalid_validators_unknown_machine.md) — diagnostic
- [invalid_validators_unknown_state_method](invalid_validators_unknown_state_method.md) — diagnostic
- [invalid_validators_wrong_receiver](invalid_validators_wrong_receiver.md) — diagnostic
- [invalid_validators_wrong_return](invalid_validators_wrong_return.md) — diagnostic
- [invalid_validators_wrong_signature](invalid_validators_wrong_signature.md) — diagnostic
- [strict_invalid_validators_relative_path](strict_invalid_validators_relative_path.md) — diagnostic

## Legacy Regression
- [invalid_legacy_machine_builder](invalid_legacy_machine_builder.md) — placeholder
- [invalid_legacy_machines_builder](invalid_legacy_machines_builder.md) — placeholder
- [invalid_legacy_state_helper_traits](invalid_legacy_state_helper_traits.md) — placeholder
- [invalid_legacy_superstate](invalid_legacy_superstate.md) — placeholder
- [invalid_legacy_transition_helper_trait](invalid_legacy_transition_helper_trait.md) — placeholder
