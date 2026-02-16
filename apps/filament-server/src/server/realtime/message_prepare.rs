use filament_core::{tokenize_markdown, MarkdownToken};

use crate::server::{auth::validate_message_content, errors::AuthFailure};

pub(crate) struct PreparedMessageBody {
    pub(crate) content: String,
    pub(crate) markdown_tokens: Vec<MarkdownToken>,
}

pub(crate) fn prepare_message_body(
    content: String,
    has_attachments: bool,
) -> Result<PreparedMessageBody, AuthFailure> {
    if content.is_empty() {
        if !has_attachments {
            return Err(AuthFailure::InvalidRequest);
        }
        return Ok(PreparedMessageBody {
            content,
            markdown_tokens: Vec::new(),
        });
    }

    validate_message_content(&content)?;
    Ok(PreparedMessageBody {
        markdown_tokens: tokenize_markdown(&content),
        content,
    })
}

#[cfg(test)]
mod tests {
    use super::prepare_message_body;
    use crate::server::errors::AuthFailure;

    #[test]
    fn rejects_empty_content_without_attachments() {
        let result = prepare_message_body(String::new(), false);
        assert!(matches!(result, Err(AuthFailure::InvalidRequest)));
    }

    #[test]
    fn accepts_empty_content_with_attachments() {
        let prepared = prepare_message_body(String::new(), true)
            .expect("empty message with attachments should be accepted");

        assert!(prepared.content.is_empty());
        assert!(prepared.markdown_tokens.is_empty());
    }

    #[test]
    fn tokenizes_non_empty_content() {
        let prepared = prepare_message_body(String::from("hello **world**"), false)
            .expect("valid message should be accepted");

        assert_eq!(prepared.content, "hello **world**");
        assert!(!prepared.markdown_tokens.is_empty());
    }

    #[test]
    fn rejects_oversized_content() {
        let oversized = "a".repeat(2001);
        let result = prepare_message_body(oversized, false);

        assert!(matches!(result, Err(AuthFailure::InvalidRequest)));
    }
}
