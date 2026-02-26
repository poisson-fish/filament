use std::time::Duration;

use crate::server::{core::AppState, errors::AuthFailure};

use super::{run_search_query_against_index, search_runtime};

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
        query: search_runtime::normalize_search_query(raw_query),
        limit,
    }
}

pub(crate) async fn run_search_blocking_with_timeout<T, F>(
    timeout: Duration,
    task: F,
) -> Result<T, AuthFailure>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AuthFailure> + Send + 'static,
{
    tokio::time::timeout(timeout, async move {
        tokio::task::spawn_blocking(task)
            .await
            .map_err(|_| AuthFailure::Internal)?
    })
    .await
    .map_err(|_| AuthFailure::InvalidRequest)?
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
    use std::{thread, time::Duration};

    use crate::server::errors::AuthFailure;

    use super::{
        build_search_query_run_input, run_search_blocking_with_timeout, SearchQueryRunInput,
    };

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

    #[tokio::test]
    async fn returns_task_result_before_timeout() {
        let result = run_search_blocking_with_timeout(Duration::from_millis(100), || Ok(42_i32))
            .await
            .expect("task should complete");

        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn fails_closed_when_timeout_expires() {
        let result = run_search_blocking_with_timeout(Duration::from_millis(20), || {
            thread::sleep(Duration::from_millis(80));
            Ok(1_i32)
        })
        .await;

        assert!(matches!(result, Err(AuthFailure::InvalidRequest)));
    }

    #[tokio::test]
    async fn maps_task_panic_to_internal_error() {
        let result: Result<i32, AuthFailure> =
            run_search_blocking_with_timeout(Duration::from_millis(100), || {
                panic!("simulated panic")
            })
            .await;

        assert!(matches!(result, Err(AuthFailure::Internal)));
    }
}
