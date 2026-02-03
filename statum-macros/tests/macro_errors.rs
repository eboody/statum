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
    t.compile_fail("tests/ui/invalid_machine_missing_state_derive.rs");
}

#[test]
fn test_invalid_transition_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_transition_no_methods.rs");
    t.compile_fail("tests/ui/invalid_transition_not_method.rs");
    t.compile_fail("tests/ui/invalid_transition_wrong_return.rs");
    t.compile_fail("tests/ui/invalid_transition_conditional.rs");
}

#[test]
fn test_invalid_validators_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_validators_missing_variant.rs");
    t.compile_fail("tests/ui/invalid_validators_wrong_return.rs");
    t.compile_fail("tests/ui/invalid_validators_wrong_signature.rs");
    t.compile_fail("tests/ui/invalid_validators_no_methods.rs");
}

#[test]
fn test_valid_macro_usage() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/valid_state_unit_only.rs");
    t.pass("tests/ui/valid_state_with_data.rs");
    t.pass("tests/ui/valid_machine_no_fields.rs");
    t.pass("tests/ui/valid_validators_sync.rs");
    t.pass("tests/ui/valid_validators_async.rs");
    t.pass("tests/ui/valid_matrix.rs");
}
