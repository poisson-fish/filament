use ulid::Ulid;

use crate::server::{
    core::MAX_ATTACHMENTS_PER_MESSAGE,
    errors::AuthFailure,
    types::{AttachmentResponse, MessageResponse},
};
use std::collections::{HashMap, HashSet};

pub(crate) fn parse_attachment_ids(value: Vec<String>) -> Result<Vec<String>, AuthFailure> {
    if value.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(AuthFailure::InvalidRequest);
    }

    let mut deduped = Vec::with_capacity(value.len());
    let mut seen = HashSet::with_capacity(value.len());
    for attachment_id in value {
        if Ulid::from_string(&attachment_id).is_err() {
            return Err(AuthFailure::InvalidRequest);
        }
        if seen.insert(attachment_id.clone()) {
            deduped.push(attachment_id);
        }
    }
    Ok(deduped)
}

pub(crate) fn validate_attachment_filename(value: String) -> Result<String, AuthFailure> {
    if value.is_empty() || value.len() > 128 {
        return Err(AuthFailure::InvalidRequest);
    }
    if value.contains('/') || value.contains('\\') || value.contains('\0') {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(value)
}

pub(crate) fn attach_message_media(
    messages: &mut [MessageResponse],
    attachment_map: &HashMap<String, Vec<AttachmentResponse>>,
) {
    for message in messages {
        message.attachments = attachment_map
            .get(&message.message_id)
            .cloned()
            .unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_attachment_ids, validate_attachment_filename};
    use crate::server::core::MAX_ATTACHMENTS_PER_MESSAGE;
    use crate::server::errors::AuthFailure;
    use ulid::Ulid;

    #[test]
    fn parse_attachment_ids_rejects_over_cap() {
        let ids = (0..=MAX_ATTACHMENTS_PER_MESSAGE)
            .map(|_| Ulid::new().to_string())
            .collect::<Vec<_>>();
        assert!(matches!(
            parse_attachment_ids(ids),
            Err(AuthFailure::InvalidRequest)
        ));
    }

    #[test]
    fn parse_attachment_ids_rejects_invalid_ulid() {
        let ids = vec![String::from("not-a-ulid")];
        assert!(matches!(
            parse_attachment_ids(ids),
            Err(AuthFailure::InvalidRequest)
        ));
    }

    #[test]
    fn parse_attachment_ids_dedupes_preserving_order() {
        let first = Ulid::new().to_string();
        let second = Ulid::new().to_string();
        let parsed = parse_attachment_ids(vec![
            first.clone(),
            second.clone(),
            first.clone(),
            second.clone(),
        ])
        .expect("ids should parse and dedupe");
        assert_eq!(parsed, vec![first, second]);
    }

    #[test]
    fn validate_attachment_filename_rejects_path_control_bytes() {
        for value in ["a/b", "a\\b", "a\0b"] {
            assert!(matches!(
                validate_attachment_filename(value.to_owned()),
                Err(AuthFailure::InvalidRequest)
            ));
        }
    }

    #[test]
    fn validate_attachment_filename_accepts_safe_name() {
        let value = String::from("report.png");
        assert_eq!(
            validate_attachment_filename(value.clone()).expect("name should be accepted"),
            value
        );
    }
}
