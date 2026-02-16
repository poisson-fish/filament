use tantivy::schema::{
    NumericOptions, Schema, TextFieldIndexing, TextOptions, STORED, STRING,
};

use crate::server::core::SearchFields;

pub(crate) fn build_search_schema() -> (Schema, SearchFields) {
    let mut schema_builder = Schema::builder();
    let message_id = schema_builder.add_text_field("message_id", STRING | STORED);
    let guild_id = schema_builder.add_text_field("guild_id", STRING | STORED);
    let channel_id = schema_builder.add_text_field("channel_id", STRING | STORED);
    let author_id = schema_builder.add_text_field("author_id", STRING | STORED);
    let created_at_unix =
        schema_builder.add_i64_field("created_at_unix", NumericOptions::default().set_stored());
    let content_options = TextOptions::default()
        .set_stored()
        .set_indexing_options(TextFieldIndexing::default().set_tokenizer("default"));
    let content = schema_builder.add_text_field("content", content_options);
    let schema = schema_builder.build();

    (
        schema,
        SearchFields {
            message_id,
            guild_id,
            channel_id,
            author_id,
            created_at_unix,
            content,
        },
    )
}

#[cfg(test)]
mod tests {
    use tantivy::schema::Type;

    use super::build_search_schema;

    #[test]
    fn build_search_schema_registers_expected_fields() {
        let (schema, fields) = build_search_schema();

        let message_field_name = schema.get_field_name(fields.message_id);
        let guild_field_name = schema.get_field_name(fields.guild_id);
        let channel_field_name = schema.get_field_name(fields.channel_id);
        let author_field_name = schema.get_field_name(fields.author_id);
        let content_field_name = schema.get_field_name(fields.content);

        assert_eq!(message_field_name, "message_id");
        assert_eq!(guild_field_name, "guild_id");
        assert_eq!(channel_field_name, "channel_id");
        assert_eq!(author_field_name, "author_id");
        assert_eq!(content_field_name, "content");
    }

    #[test]
    fn build_search_schema_marks_created_at_as_i64() {
        let (schema, fields) = build_search_schema();

        let entry = schema.get_field_entry(fields.created_at_unix);

        assert_eq!(entry.field_type().value_type(), Type::I64);
    }
}
