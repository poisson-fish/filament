use crate::server::{errors::AuthFailure, types::SearchQuery};

use super::search_validation::validate_search_query_limits;

pub(crate) fn normalize_search_query(raw_query: &str) -> String {
    raw_query.trim().to_owned()
}

pub(crate) fn effective_search_limit(
    requested_limit: Option<usize>,
    default_limit: usize,
) -> usize {
    requested_limit.unwrap_or(default_limit)
}

pub(crate) fn validate_search_query_request(
    query: &SearchQuery,
    default_limit: usize,
    max_chars: usize,
    max_limit: usize,
) -> Result<(), AuthFailure> {
    let raw = normalize_search_query(&query.q);
    let limit = effective_search_limit(query.limit, default_limit);
    validate_search_query_limits(&raw, limit, max_chars, max_limit)
}

#[cfg(test)]
mod tests {
    use super::{effective_search_limit, normalize_search_query, validate_search_query_request};
    use crate::server::{errors::AuthFailure, types::SearchQuery};

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

    #[test]
    fn validate_search_query_request_rejects_blank_query_fail_closed() {
        let query = SearchQuery {
            q: String::from("   \n"),
            limit: Some(10),
            channel_id: None,
        };

        let error = validate_search_query_request(&query, 25, 500, 100)
            .expect_err("blank query should fail closed");
        assert!(matches!(error, AuthFailure::InvalidRequest));
    }

    #[test]
    fn validate_search_query_request_uses_default_limit_when_missing() {
        let query = SearchQuery {
            q: String::from("hello"),
            limit: None,
            channel_id: None,
        };

        let result = validate_search_query_request(&query, 25, 500, 100);
        assert!(result.is_ok());
    }
}
