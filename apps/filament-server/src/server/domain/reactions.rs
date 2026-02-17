use filament_core::UserId;

use crate::server::{
    core::MAX_REACTION_EMOJI_CHARS,
    errors::AuthFailure,
    types::{MessageResponse, ReactionResponse},
};
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub(crate) struct ReactionCountDbRow {
    pub(crate) message_id: String,
    pub(crate) emoji: String,
    pub(crate) count: i64,
}

pub(crate) fn attach_message_reactions(
    messages: &mut [MessageResponse],
    reaction_map: &HashMap<String, Vec<ReactionResponse>>,
) {
    for message in messages {
        message.reactions = reaction_map
            .get(&message.message_id)
            .cloned()
            .unwrap_or_default();
    }
}

pub(crate) fn reaction_summaries_from_users(
    reactions: &HashMap<String, HashSet<UserId>>,
) -> Vec<ReactionResponse> {
    let mut summaries = Vec::with_capacity(reactions.len());
    for (emoji, users) in reactions {
        summaries.push(ReactionResponse {
            emoji: emoji.clone(),
            count: users.len(),
        });
    }
    summaries.sort_by(|left, right| left.emoji.cmp(&right.emoji));
    summaries
}

pub(crate) fn validate_reaction_emoji(value: &str) -> Result<(), AuthFailure> {
    if value.is_empty() || value.chars().count() > MAX_REACTION_EMOJI_CHARS {
        return Err(AuthFailure::InvalidRequest);
    }
    if value.chars().any(char::is_whitespace) {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(())
}

pub(crate) fn reaction_map_from_counts(
    counts: Vec<(String, String, i64)>,
) -> Result<HashMap<String, Vec<ReactionResponse>>, AuthFailure> {
    let mut by_message: HashMap<String, Vec<ReactionResponse>> = HashMap::new();
    for (message_id, emoji, count) in counts {
        by_message
            .entry(message_id)
            .or_default()
            .push(ReactionResponse {
                emoji,
                count: usize::try_from(count).map_err(|_| AuthFailure::Internal)?,
            });
    }
    for reactions in by_message.values_mut() {
        reactions.sort_by(|left, right| left.emoji.cmp(&right.emoji));
    }
    Ok(by_message)
}

pub(crate) fn reaction_count_from_db_fields(
    message_id: String,
    emoji: String,
    count: i64,
) -> Result<(String, String, i64), AuthFailure> {
    if count < 0 {
        return Err(AuthFailure::Internal);
    }
    Ok((message_id, emoji, count))
}

pub(crate) fn reaction_map_from_db_rows(
    rows: Vec<ReactionCountDbRow>,
) -> Result<HashMap<String, Vec<ReactionResponse>>, AuthFailure> {
    let mut counts = Vec::with_capacity(rows.len());
    for row in rows {
        counts.push(reaction_count_from_db_fields(
            row.message_id,
            row.emoji,
            row.count,
        )?);
    }
    reaction_map_from_counts(counts)
}

#[cfg(test)]
mod tests {
    use super::{
        reaction_count_from_db_fields,
        reaction_map_from_db_rows,
        reaction_map_from_counts, reaction_summaries_from_users,
        validate_reaction_emoji,
    };
    use crate::server::errors::AuthFailure;
    use filament_core::UserId;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn reaction_summaries_from_users_sorts_by_emoji() {
        let mut reactions: HashMap<String, HashSet<UserId>> = HashMap::new();
        reactions.insert(String::from("😄"), HashSet::from([UserId::new()]));
        reactions.insert(
            String::from("😀"),
            HashSet::from([UserId::new(), UserId::new()]),
        );

        let summaries = reaction_summaries_from_users(&reactions);
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].emoji, "😀");
        assert_eq!(summaries[0].count, 2);
        assert_eq!(summaries[1].emoji, "😄");
        assert_eq!(summaries[1].count, 1);
    }

    #[test]
    fn validate_reaction_emoji_rejects_whitespace_and_empty() {
        for value in ["", "😀 😀", " \t"] {
            assert!(matches!(
                validate_reaction_emoji(value),
                Err(AuthFailure::InvalidRequest)
            ));
        }
    }

    #[test]
    fn validate_reaction_emoji_accepts_compact_emoji() {
        assert!(validate_reaction_emoji("🔥").is_ok());
    }

    #[test]
    fn reaction_map_from_counts_sorts_reactions_and_groups_by_message() {
        let map = reaction_map_from_counts(vec![
            (String::from("m1"), String::from("😄"), 1),
            (String::from("m1"), String::from("😀"), 2),
            (String::from("m2"), String::from("🔥"), 3),
        ])
        .expect("counts should map");

        let m1 = map.get("m1").expect("m1 should exist");
        assert_eq!(m1.len(), 2);
        assert_eq!(m1[0].emoji, "😀");
        assert_eq!(m1[1].emoji, "😄");

        let m2 = map.get("m2").expect("m2 should exist");
        assert_eq!(m2.len(), 1);
        assert_eq!(m2[0].emoji, "🔥");
        assert_eq!(m2[0].count, 3);
    }

    #[test]
    fn reaction_map_from_counts_rejects_negative_count_fail_closed() {
        assert!(matches!(
            reaction_map_from_counts(vec![(
                String::from("m1"),
                String::from("😀"),
                -1,
            )]),
            Err(AuthFailure::Internal)
        ));
    }

    #[test]
    fn reaction_count_from_db_fields_accepts_non_negative_count() {
        let mapped = reaction_count_from_db_fields(
            String::from("m1"),
            String::from("🔥"),
            2,
        )
        .expect("non-negative count should map");
        assert_eq!(mapped.0, "m1");
        assert_eq!(mapped.1, "🔥");
        assert_eq!(mapped.2, 2);
    }

    #[test]
    fn reaction_count_from_db_fields_rejects_negative_count_fail_closed() {
        assert!(matches!(
            reaction_count_from_db_fields(
                String::from("m1"),
                String::from("🔥"),
                -1,
            ),
            Err(AuthFailure::Internal)
        ));
    }

    #[test]
    fn reaction_map_from_db_rows_maps_and_sorts_by_message() {
        let map = reaction_map_from_db_rows(vec![
            super::ReactionCountDbRow {
                message_id: String::from("m1"),
                emoji: String::from("😄"),
                count: 1,
            },
            super::ReactionCountDbRow {
                message_id: String::from("m1"),
                emoji: String::from("😀"),
                count: 2,
            },
            super::ReactionCountDbRow {
                message_id: String::from("m2"),
                emoji: String::from("🔥"),
                count: 3,
            },
        ])
        .expect("rows should map");

        let m1 = map.get("m1").expect("m1 should exist");
        assert_eq!(m1.len(), 2);
        assert_eq!(m1[0].emoji, "😀");
        assert_eq!(m1[1].emoji, "😄");

        let m2 = map.get("m2").expect("m2 should exist");
        assert_eq!(m2.len(), 1);
        assert_eq!(m2[0].emoji, "🔥");
        assert_eq!(m2[0].count, 3);
    }

    #[test]
    fn reaction_map_from_db_rows_rejects_negative_count_fail_closed() {
        assert!(matches!(
            reaction_map_from_db_rows(vec![super::ReactionCountDbRow {
                message_id: String::from("m1"),
                emoji: String::from("🔥"),
                count: -1,
            }]),
            Err(AuthFailure::Internal)
        ));
    }
}
