use std::collections::HashSet;

use crate::server::{core::AppState, errors::AuthFailure};

use super::{collect_index_message_ids_for_guild_from_index, run_search_blocking_with_timeout};

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchIndexLookupInput {
    guild_id: String,
    max_docs: usize,
}

fn build_search_index_lookup_input(guild_id: &str, max_docs: usize) -> SearchIndexLookupInput {
    SearchIndexLookupInput {
        guild_id: guild_id.to_owned(),
        max_docs,
    }
}

pub(crate) async fn collect_index_message_ids_for_guild(
    state: &AppState,
    guild_id: &str,
    max_docs: usize,
) -> Result<HashSet<String>, AuthFailure> {
    let input = build_search_index_lookup_input(guild_id, max_docs);
    let search_state = state.search.state.clone();
    let timeout = state.runtime.search_query_timeout;

    run_search_blocking_with_timeout(timeout, move || {
        collect_index_message_ids_for_guild_from_index(
            &search_state,
            &input.guild_id,
            input.max_docs,
        )
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::{build_search_index_lookup_input, SearchIndexLookupInput};

    #[test]
    fn build_search_index_lookup_input_copies_values() {
        let input = build_search_index_lookup_input("guild-1", 55);

        assert_eq!(
            input,
            SearchIndexLookupInput {
                guild_id: String::from("guild-1"),
                max_docs: 55,
            }
        );
    }

    #[test]
    fn build_search_index_lookup_input_preserves_empty_guild_id() {
        let input = build_search_index_lookup_input("", 1);

        assert_eq!(input.guild_id, "");
        assert_eq!(input.max_docs, 1);
    }
}
