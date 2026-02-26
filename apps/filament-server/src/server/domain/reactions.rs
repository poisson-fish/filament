use filament_core::UserId;
use sqlx::{PgPool, Row};

use crate::server::{
    core::{
        MAX_REACTIONS_PER_MESSAGE, MAX_REACTION_EMOJI_CHARS, MAX_REACTOR_USER_IDS_PER_REACTION,
    },
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

#[derive(Debug)]
struct ReactionUserDbRow {
    message_id: String,
    emoji: String,
    user_id: String,
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
    viewer_user_id: Option<UserId>,
) -> Vec<ReactionResponse> {
    let mut summaries = Vec::with_capacity(reactions.len());
    for (emoji, users) in reactions {
        let mut reactor_user_ids: Vec<String> = users.iter().map(ToString::to_string).collect();
        reactor_user_ids.sort();
        reactor_user_ids.truncate(MAX_REACTOR_USER_IDS_PER_REACTION);

        let reacted_by_me = viewer_user_id
            .as_ref()
            .is_some_and(|viewer| users.contains(viewer));
        summaries.push(ReactionResponse {
            emoji: emoji.clone(),
            count: users.len(),
            reacted_by_me,
            reactor_user_ids,
        });
    }
    finalize_reaction_entries(&mut summaries);
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

fn finalize_reaction_entries(entries: &mut Vec<ReactionResponse>) {
    entries.sort_by(|left, right| left.emoji.cmp(&right.emoji));
    if entries.len() > MAX_REACTIONS_PER_MESSAGE {
        entries.truncate(MAX_REACTIONS_PER_MESSAGE);
    }
}

fn reaction_map_from_counts_with_metadata(
    counts: Vec<(String, String, i64)>,
    reactors_by_message_emoji: &HashMap<(String, String), Vec<UserId>>,
    reacted_by_viewer: &HashSet<(String, String)>,
) -> Result<HashMap<String, Vec<ReactionResponse>>, AuthFailure> {
    let mut by_message: HashMap<String, Vec<ReactionResponse>> = HashMap::new();
    for (message_id, emoji, count) in counts {
        let count = usize::try_from(count).map_err(|_| AuthFailure::Internal)?;
        let mut reactor_user_ids = reactors_by_message_emoji
            .get(&(message_id.clone(), emoji.clone()))
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|user_id| user_id.to_string())
            .collect::<Vec<_>>();
        reactor_user_ids.sort();
        reactor_user_ids.truncate(MAX_REACTOR_USER_IDS_PER_REACTION);

        by_message
            .entry(message_id.clone())
            .or_default()
            .push(ReactionResponse {
                emoji: emoji.clone(),
                count,
                reacted_by_me: reacted_by_viewer.contains(&(message_id, emoji)),
                reactor_user_ids,
            });
    }
    for reactions in by_message.values_mut() {
        finalize_reaction_entries(reactions);
    }
    Ok(by_message)
}

pub(crate) fn reaction_map_from_counts(
    counts: Vec<(String, String, i64)>,
) -> Result<HashMap<String, Vec<ReactionResponse>>, AuthFailure> {
    reaction_map_from_counts_with_metadata(counts, &HashMap::new(), &HashSet::new())
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

fn parse_reaction_user_id(
    message_id: String,
    emoji: String,
    user_id: String,
) -> Result<(String, String, UserId), AuthFailure> {
    let user_id = UserId::try_from(user_id).map_err(|_| AuthFailure::Internal)?;
    Ok((message_id, emoji, user_id))
}

fn reaction_users_map_from_db_rows(
    rows: Vec<ReactionUserDbRow>,
) -> Result<HashMap<(String, String), Vec<UserId>>, AuthFailure> {
    let mut by_message_emoji: HashMap<(String, String), Vec<UserId>> = HashMap::new();
    for row in rows {
        let (message_id, emoji, user_id) =
            parse_reaction_user_id(row.message_id, row.emoji, row.user_id)?;
        let users = by_message_emoji.entry((message_id, emoji)).or_default();
        users.push(user_id);
    }
    for users in by_message_emoji.values_mut() {
        users.sort_by(|left, right| left.to_string().cmp(&right.to_string()));
        users.truncate(MAX_REACTOR_USER_IDS_PER_REACTION);
    }
    Ok(by_message_emoji)
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

pub(crate) async fn reaction_map_for_messages_db(
    pool: &PgPool,
    guild_id: &str,
    channel_id: Option<&str>,
    message_ids: &[String],
    viewer_user_id: Option<UserId>,
) -> Result<HashMap<String, Vec<ReactionResponse>>, AuthFailure> {
    if message_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = if let Some(channel_id) = channel_id {
        sqlx::query(
            "SELECT message_id, emoji, COUNT(*) AS count
             FROM message_reactions
             WHERE guild_id = $1 AND channel_id = $2 AND message_id = ANY($3::text[])
             GROUP BY message_id, emoji",
        )
        .bind(guild_id)
        .bind(channel_id)
        .bind(message_ids)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
    } else {
        sqlx::query(
            "SELECT message_id, emoji, COUNT(*) AS count
             FROM message_reactions
             WHERE guild_id = $1 AND message_id = ANY($2::text[])
             GROUP BY message_id, emoji",
        )
        .bind(guild_id)
        .bind(message_ids)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
    };

    let mut count_rows = Vec::with_capacity(rows.len());
    for row in rows {
        count_rows.push(ReactionCountDbRow {
            message_id: row
                .try_get("message_id")
                .map_err(|_| AuthFailure::Internal)?,
            emoji: row.try_get("emoji").map_err(|_| AuthFailure::Internal)?,
            count: row.try_get("count").map_err(|_| AuthFailure::Internal)?,
        });
    }

    let user_limit =
        i64::try_from(MAX_REACTOR_USER_IDS_PER_REACTION).map_err(|_| AuthFailure::Internal)?;
    let user_rows = if let Some(channel_id) = channel_id {
        sqlx::query(
            "SELECT message_id, emoji, user_id
             FROM (
               SELECT message_id, emoji, user_id,
                      ROW_NUMBER() OVER (PARTITION BY message_id, emoji ORDER BY user_id) AS rank
               FROM message_reactions
               WHERE guild_id = $1 AND channel_id = $2 AND message_id = ANY($3::text[])
             ) ranked
             WHERE rank <= $4
             ORDER BY message_id, emoji, user_id",
        )
        .bind(guild_id)
        .bind(channel_id)
        .bind(message_ids)
        .bind(user_limit)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
    } else {
        sqlx::query(
            "SELECT message_id, emoji, user_id
             FROM (
               SELECT message_id, emoji, user_id,
                      ROW_NUMBER() OVER (PARTITION BY message_id, emoji ORDER BY user_id) AS rank
               FROM message_reactions
               WHERE guild_id = $1 AND message_id = ANY($2::text[])
             ) ranked
             WHERE rank <= $3
             ORDER BY message_id, emoji, user_id",
        )
        .bind(guild_id)
        .bind(message_ids)
        .bind(user_limit)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthFailure::Internal)?
    };

    let mut reactor_rows = Vec::with_capacity(user_rows.len());
    for row in user_rows {
        reactor_rows.push(ReactionUserDbRow {
            message_id: row
                .try_get("message_id")
                .map_err(|_| AuthFailure::Internal)?,
            emoji: row.try_get("emoji").map_err(|_| AuthFailure::Internal)?,
            user_id: row.try_get("user_id").map_err(|_| AuthFailure::Internal)?,
        });
    }
    let reactors_by_message_emoji = reaction_users_map_from_db_rows(reactor_rows)?;

    let mut reacted_by_viewer = HashSet::new();
    if let Some(viewer_user_id) = viewer_user_id {
        let viewer_user_id = viewer_user_id.to_string();
        let viewer_rows = if let Some(channel_id) = channel_id {
            sqlx::query(
                "SELECT DISTINCT message_id, emoji
                 FROM message_reactions
                 WHERE guild_id = $1
                   AND channel_id = $2
                   AND message_id = ANY($3::text[])
                   AND user_id = $4",
            )
            .bind(guild_id)
            .bind(channel_id)
            .bind(message_ids)
            .bind(viewer_user_id)
            .fetch_all(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?
        } else {
            sqlx::query(
                "SELECT DISTINCT message_id, emoji
                 FROM message_reactions
                 WHERE guild_id = $1
                   AND message_id = ANY($2::text[])
                   AND user_id = $3",
            )
            .bind(guild_id)
            .bind(message_ids)
            .bind(viewer_user_id)
            .fetch_all(pool)
            .await
            .map_err(|_| AuthFailure::Internal)?
        };

        for row in viewer_rows {
            reacted_by_viewer.insert((
                row.try_get("message_id")
                    .map_err(|_| AuthFailure::Internal)?,
                row.try_get("emoji").map_err(|_| AuthFailure::Internal)?,
            ));
        }
    }

    let mut counts = Vec::with_capacity(count_rows.len());
    for row in count_rows {
        counts.push(reaction_count_from_db_fields(
            row.message_id,
            row.emoji,
            row.count,
        )?);
    }

    reaction_map_from_counts_with_metadata(counts, &reactors_by_message_emoji, &reacted_by_viewer)
}

#[cfg(test)]
mod tests {
    use super::{
        reaction_count_from_db_fields, reaction_map_for_messages_db, reaction_map_from_counts,
        reaction_map_from_db_rows, reaction_summaries_from_users, validate_reaction_emoji,
    };
    use crate::server::{
        core::{MAX_REACTIONS_PER_MESSAGE, MAX_REACTOR_USER_IDS_PER_REACTION},
        errors::AuthFailure,
    };
    use filament_core::UserId;
    use sqlx::PgPool;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn reaction_summaries_from_users_sorts_by_emoji() {
        let viewer = UserId::new();
        let mut reactions: HashMap<String, HashSet<UserId>> = HashMap::new();
        reactions.insert(String::from("😄"), HashSet::from([viewer]));
        reactions.insert(
            String::from("😀"),
            HashSet::from([UserId::new(), UserId::new()]),
        );

        let summaries = reaction_summaries_from_users(&reactions, Some(viewer));
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].emoji, "😀");
        assert_eq!(summaries[0].count, 2);
        assert!(!summaries[0].reacted_by_me);
        assert_eq!(summaries[1].emoji, "😄");
        assert_eq!(summaries[1].count, 1);
        assert!(summaries[1].reacted_by_me);
        assert_eq!(summaries[1].reactor_user_ids, vec![viewer.to_string()]);
    }

    #[test]
    fn reaction_summaries_from_users_caps_per_reaction_users_and_reactions() {
        let mut reactions: HashMap<String, HashSet<UserId>> = HashMap::new();
        for index in 0..(MAX_REACTIONS_PER_MESSAGE + 10) {
            let emoji = format!("e{index:03}");
            let mut users = HashSet::new();
            for _ in 0..(MAX_REACTOR_USER_IDS_PER_REACTION + 10) {
                users.insert(UserId::new());
            }
            reactions.insert(emoji, users);
        }

        let summaries = reaction_summaries_from_users(&reactions, None);
        assert_eq!(summaries.len(), MAX_REACTIONS_PER_MESSAGE);
        assert!(summaries
            .iter()
            .all(|entry| entry.reactor_user_ids.len() <= MAX_REACTOR_USER_IDS_PER_REACTION));
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
        assert!(!m1[0].reacted_by_me);
        assert!(m1[0].reactor_user_ids.is_empty());
        assert_eq!(m1[1].emoji, "😄");

        let m2 = map.get("m2").expect("m2 should exist");
        assert_eq!(m2.len(), 1);
        assert_eq!(m2[0].emoji, "🔥");
        assert_eq!(m2[0].count, 3);
    }

    #[test]
    fn reaction_map_from_counts_rejects_negative_count_fail_closed() {
        assert!(matches!(
            reaction_map_from_counts(vec![(String::from("m1"), String::from("😀"), -1,)]),
            Err(AuthFailure::Internal)
        ));
    }

    #[test]
    fn reaction_count_from_db_fields_accepts_non_negative_count() {
        let mapped = reaction_count_from_db_fields(String::from("m1"), String::from("🔥"), 2)
            .expect("non-negative count should map");
        assert_eq!(mapped.0, "m1");
        assert_eq!(mapped.1, "🔥");
        assert_eq!(mapped.2, 2);
    }

    #[test]
    fn reaction_count_from_db_fields_rejects_negative_count_fail_closed() {
        assert!(matches!(
            reaction_count_from_db_fields(String::from("m1"), String::from("🔥"), -1,),
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

    #[tokio::test]
    async fn reaction_map_for_messages_db_short_circuits_empty_ids() {
        let pool = PgPool::connect_lazy("postgres://local/ignored")
            .expect("lazy pool should build without network");
        let mapped = reaction_map_for_messages_db(&pool, "guild", None, &[], None)
            .await
            .expect("empty ids should short-circuit");
        assert!(mapped.is_empty());
    }
}
