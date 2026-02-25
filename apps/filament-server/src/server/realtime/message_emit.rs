use crate::server::{
    auth::channel_key,
    core::{AppState, SearchOperation},
    errors::AuthFailure,
    gateway_events,
    metrics::record_gateway_event_serialize_error,
    types::MessageResponse,
};

use super::{broadcast_channel_event, enqueue_search_operation, indexed_message_from_response};

pub(crate) fn message_upsert_operation(response: &MessageResponse) -> SearchOperation {
    SearchOperation::Upsert(indexed_message_from_response(response))
}

pub(crate) async fn emit_message_create_and_index(
    state: &AppState,
    guild_id: &str,
    channel_id: &str,
    response: &MessageResponse,
) -> Result<(), AuthFailure> {
    if let Ok(event) = gateway_events::try_message_create(response) {
        broadcast_channel_event(state, &channel_key(guild_id, channel_id), &event).await;
    } else {
        record_gateway_event_serialize_error("channel", gateway_events::MESSAGE_CREATE_EVENT);
        tracing::warn!(
            guild_id,
            channel_id,
            "dropped message_create outbound event because serialization failed"
        );
    }
    enqueue_search_operation(state, message_upsert_operation(response), true).await
}

#[cfg(test)]
mod tests {
    use filament_core::MarkdownToken;

    use super::message_upsert_operation;
    use crate::server::{core::SearchOperation, types::MessageResponse};

    #[test]
    fn message_upsert_operation_maps_response_fields() {
        let response = MessageResponse {
            message_id: String::from("m1"),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            author_id: String::from("u1"),
            content: String::from("hello"),
            markdown_tokens: vec![MarkdownToken::Text {
                text: String::from("hello"),
            }],
            attachments: Vec::new(),
            reactions: Vec::new(),
            created_at_unix: 42,
        };

        let op = message_upsert_operation(&response);
        let SearchOperation::Upsert(doc) = op else {
            panic!("expected upsert operation");
        };

        assert_eq!(doc.message_id, "m1");
        assert_eq!(doc.guild_id, "g1");
        assert_eq!(doc.channel_id, "c1");
        assert_eq!(doc.author_id, "u1");
        assert_eq!(doc.content, "hello");
        assert_eq!(doc.created_at_unix, 42);
    }
}
