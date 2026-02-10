use filament_core::project_name;

#[test]
fn core_smoke_test() {
    assert_eq!(project_name(), "filament");
}
