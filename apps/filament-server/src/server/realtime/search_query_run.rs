use crate::server::{core::AppState, errors::AuthFailure};

use super::{
    run_search_blocking_with_timeout, run_search_query_against_index,
};
use super::search_query_input::normalize_search_query;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchQueryRunInput {
    guild_id: String,
    channel_id: Option<String>,
    query: String,
    limit: usize,
}

fn build_search_query_run_input(
    guild_id: &str,
    channel_id: Option<&str>,
    raw_query: &str,
    limit: usize,
) -> SearchQueryRunInput {
    SearchQueryRunInput {
        guild_id: guild_id.to_owned(),
        channel_id: channel_id.map(ToOwned::to_owned),
        query: normalize_search_query(raw_query),
        limit,
    }
}

pub(crate) async fn run_search_query(
    state: &AppState,
    guild_id: &str,
    channel_id: Option<&str>,
    raw_query: &str,
    limit: usize,
) -> Result<Vec<String>, AuthFailure> {
    let input = build_search_query_run_input(guild_id, channel_id, raw_query, limit);
    let search_state = state.search.state.clone();
    let timeout = state.runtime.search_query_timeout;

    run_search_blocking_with_timeout(timeout, move || {
        run_search_query_against_index(
            &search_state,
            &input.guild_id,
            input.channel_id.as_deref(),
            &input.query,
            input.limit,
        )
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::{build_search_query_run_input, SearchQueryRunInput};

    #[test]
    fn build_search_query_run_input_trims_and_copies_values() {
        let input =
            build_search_query_run_input("guild-1", Some("channel-9"), "  hello world  ", 17);

        assert_eq!(
            input,
            SearchQueryRunInput {
                guild_id: String::from("guild-1"),
                channel_id: Some(String::from("channel-9")),
                query: String::from("hello world"),
                limit: 17,
            }
        );
    }

    #[test]
    fn build_search_query_run_input_handles_global_channel_scope() {
        let input = build_search_query_run_input("guild-2", None, "query", 5);

        assert_eq!(input.channel_id, None);
        assert_eq!(input.guild_id, "guild-2");
        assert_eq!(input.query, "query");
        assert_eq!(input.limit, 5);
    }
}
