use filament_core::UserId;

use crate::server::{
    core::MAX_REACTION_EMOJI_CHARS,
    errors::AuthFailure,
    types::{MessageResponse, ReactionResponse},
};
use std::collections::{HashMap, HashSet};

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

#[cfg(test)]
mod tests {
    use super::{reaction_summaries_from_users, validate_reaction_emoji};
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
}
