use crate::server::{
    core::{MAX_SEARCH_FUZZY, MAX_SEARCH_TERMS, MAX_SEARCH_WILDCARDS},
    errors::AuthFailure,
};

pub(crate) fn validate_search_query_limits(
    raw_query: &str,
    limit: usize,
    max_query_chars: usize,
    max_result_limit: usize,
) -> Result<(), AuthFailure> {
    if raw_query.is_empty() || raw_query.len() > max_query_chars {
        return Err(AuthFailure::InvalidRequest);
    }
    if limit == 0 || limit > max_result_limit {
        return Err(AuthFailure::InvalidRequest);
    }
    if raw_query.split_whitespace().count() > MAX_SEARCH_TERMS {
        return Err(AuthFailure::InvalidRequest);
    }
    let wildcard_count = raw_query.matches('*').count() + raw_query.matches('?').count();
    if wildcard_count > MAX_SEARCH_WILDCARDS {
        return Err(AuthFailure::InvalidRequest);
    }
    if raw_query.matches('~').count() > MAX_SEARCH_FUZZY {
        return Err(AuthFailure::InvalidRequest);
    }
    if raw_query.contains(':') {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_search_query_limits;

    #[test]
    fn accepts_safe_query_within_limits() {
        let result = validate_search_query_limits("release notes", 20, 256, 50);
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_empty_and_overlong_queries() {
        assert!(validate_search_query_limits("", 20, 256, 50).is_err());
        let too_long = "a".repeat(257);
        assert!(validate_search_query_limits(&too_long, 20, 256, 50).is_err());
    }

    #[test]
    fn rejects_invalid_limit_values() {
        assert!(validate_search_query_limits("ok", 0, 256, 50).is_err());
        assert!(validate_search_query_limits("ok", 51, 256, 50).is_err());
    }

    #[test]
    fn rejects_abusive_query_patterns() {
        assert!(validate_search_query_limits(
            "a b c d e f g h i j k l m n o p q r s t u",
            20,
            256,
            50
        )
        .is_err());
        assert!(validate_search_query_limits("a*b?c*d?e*f", 20, 256, 50).is_err());
        assert!(validate_search_query_limits("a~b~c~", 20, 256, 50).is_err());
        assert!(validate_search_query_limits("author:alice", 20, 256, 50).is_err());
    }
}
