pub(crate) fn normalize_search_query(raw_query: &str) -> String {
    raw_query.trim().to_owned()
}

pub(crate) fn effective_search_limit(
    requested_limit: Option<usize>,
    default_limit: usize,
) -> usize {
    requested_limit.unwrap_or(default_limit)
}

#[cfg(test)]
mod tests {
    use super::{effective_search_limit, normalize_search_query};

    #[test]
    fn normalize_search_query_trims_surrounding_whitespace() {
        assert_eq!(normalize_search_query("  hello world  \n"), "hello world");
    }

    #[test]
    fn normalize_search_query_preserves_internal_whitespace() {
        assert_eq!(normalize_search_query("  hello   world  "), "hello   world");
    }

    #[test]
    fn effective_search_limit_uses_default_when_missing() {
        assert_eq!(effective_search_limit(None, 25), 25);
    }

    #[test]
    fn effective_search_limit_uses_requested_when_present() {
        assert_eq!(effective_search_limit(Some(10), 25), 10);
    }
}
