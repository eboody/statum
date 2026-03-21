#[test]
fn child_interplay_surface_compiles_and_rejects_invalid_wrappers() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/valid_child_interplay.rs");
    t.compile_fail("tests/ui/invalid_child_wrapper_multiple_fields.rs");
    t.compile_fail("tests/ui/invalid_child_wrapper_outside_module.rs");
    t.compile_fail("tests/ui/invalid_child_wrapper_tuple_struct.rs");
}
