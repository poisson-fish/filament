use crate::server::{
    core::IndexedMessage,
    types::MessageResponse,
};

pub(crate) fn indexed_message_from_response(message: &MessageResponse) -> IndexedMessage {
    IndexedMessage {
        message_id: message.message_id.clone(),
        guild_id: message.guild_id.clone(),
        channel_id: message.channel_id.clone(),
        author_id: message.author_id.clone(),
        created_at_unix: message.created_at_unix,
        content: message.content.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::indexed_message_from_response;
    use crate::server::types::MessageResponse;

    #[test]
    fn indexed_message_from_response_maps_all_fields() {
        let response = MessageResponse {
            message_id: String::from("m1"),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            author_id: String::from("u1"),
            content: String::from("hello"),
            markdown_tokens: Vec::new(),
            attachments: Vec::new(),
            reactions: Vec::new(),
            created_at_unix: 42,
        };

        let indexed = indexed_message_from_response(&response);

        assert_eq!(indexed.message_id, "m1");
        assert_eq!(indexed.guild_id, "g1");
        assert_eq!(indexed.channel_id, "c1");
        assert_eq!(indexed.author_id, "u1");
        assert_eq!(indexed.content, "hello");
        assert_eq!(indexed.created_at_unix, 42);
    }
}