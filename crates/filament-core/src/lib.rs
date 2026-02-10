#![forbid(unsafe_code)]

/// Returns the project code name.
#[must_use]
pub const fn project_name() -> &'static str {
    "filament"
}

#[cfg(test)]
mod tests {
    use super::project_name;

    #[test]
    fn project_name_is_stable() {
        assert_eq!(project_name(), "filament");
    }
}
