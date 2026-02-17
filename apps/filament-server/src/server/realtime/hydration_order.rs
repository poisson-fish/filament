use std::collections::HashMap;

use crate::server::types::MessageResponse;

pub(crate) fn collect_hydrated_in_request_order(
    mut by_id: HashMap<String, MessageResponse>,
    message_ids: &[String],
) -> Vec<MessageResponse> {
    let mut hydrated = Vec::with_capacity(message_ids.len());
    for message_id in message_ids {
        if let Some(message) = by_id.remove(message_id) {
            hydrated.push(message);
        }
    }
    hydrated
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use filament_core::MarkdownToken;

    use super::collect_hydrated_in_request_order;
    use crate::server::types::MessageResponse;

    fn message(id: &str) -> MessageResponse {
        MessageResponse {
            message_id: id.to_owned(),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            author_id: String::from("u1"),
            content: format!("content-{id}"),
            markdown_tokens: vec![MarkdownToken::Text {
                text: String::from("content"),
            }],
            attachments: Vec::new(),
            reactions: Vec::new(),
            created_at_unix: 1,
        }
    }

    #[test]
    fn returns_messages_in_requested_order() {
        let by_id = HashMap::from([
            (String::from("m1"), message("m1")),
            (String::from("m2"), message("m2")),
            (String::from("m3"), message("m3")),
        ]);

        let ordered =
            collect_hydrated_in_request_order(by_id, &[String::from("m3"), String::from("m1")]);

        let ids: Vec<String> = ordered.into_iter().map(|entry| entry.message_id).collect();
        assert_eq!(ids, vec![String::from("m3"), String::from("m1")]);
    }

    #[test]
    fn skips_missing_ids_fail_closed_to_available_rows() {
        let by_id = HashMap::from([(String::from("m1"), message("m1"))]);

        let ordered = collect_hydrated_in_request_order(
            by_id,
            &[String::from("m2"), String::from("m1"), String::from("m3")],
        );

        assert_eq!(ordered.len(), 1);
        assert_eq!(ordered[0].message_id, "m1");
    }
}
