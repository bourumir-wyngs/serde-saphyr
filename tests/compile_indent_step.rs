#[test]
fn serializer_options_indent_step_range_is_enforced_for_literals() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/indent_step_ok.rs");
    t.compile_fail("tests/ui/indent_step_zero.rs");
    t.compile_fail("tests/ui/indent_step_too_large.rs");
    t.compile_fail("tests/ui/indent_step_huge.rs");
}
