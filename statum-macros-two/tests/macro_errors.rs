#[test]
fn test_invalid_state_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_state_struct.rs");
    t.compile_fail("tests/ui/invalid_state_variant.rs");
    t.compile_fail("tests/ui/empty_state_enum.rs");
}

#[test]
fn test_invalid_machine_usage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_machine_no_generics.rs");
    t.compile_fail("tests/ui/invalid_machine_no_state.rs");
}
