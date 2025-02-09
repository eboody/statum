#[test]
fn test_invalid_state_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_state_not_enum.rs");
    t.compile_fail("tests/ui/invalid_state_empty_enum.rs");
    t.compile_fail("tests/ui/invalid_state_struct_variant.rs");
    t.compile_fail("tests/ui/invalid_state_with_generics.rs");
}

#[test]
fn test_invalid_machine_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_machine_not_struct.rs");
    t.compile_fail("tests/ui/invalid_machine_no_state_generic.rs");
}
