use ulid::Ulid;

use crate::server::{
    core::{AttachmentRecord, MAX_ATTACHMENTS_PER_MESSAGE},
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

pub(crate) fn attachment_response_from_record(record: &AttachmentRecord) -> AttachmentResponse {
    AttachmentResponse {
        attachment_id: record.attachment_id.clone(),
        guild_id: record.guild_id.clone(),
        channel_id: record.channel_id.clone(),
        owner_id: record.owner_id.to_string(),
        filename: record.filename.clone(),
        mime_type: record.mime_type.clone(),
        size_bytes: record.size_bytes,
        sha256_hex: record.sha256_hex.clone(),
    }
}

pub(crate) fn attachment_map_from_records<'a>(
    records: impl Iterator<Item = &'a AttachmentRecord>,
    guild_id: &str,
    channel_id: Option<&str>,
    message_ids: &[String],
) -> HashMap<String, Vec<AttachmentResponse>> {
    if message_ids.is_empty() {
        return HashMap::new();
    }

    let wanted: HashSet<&str> = message_ids.iter().map(String::as_str).collect();
    let mut by_message: HashMap<String, Vec<AttachmentResponse>> = HashMap::new();
    for record in records {
        let Some(message_id) = record.message_id.as_deref() else {
            continue;
        };
        if record.guild_id != guild_id {
            continue;
        }
        if channel_id.is_some_and(|cid| cid != record.channel_id) {
            continue;
        }
        if !wanted.contains(message_id) {
            continue;
        }
        by_message
            .entry(message_id.to_owned())
            .or_default()
            .push(attachment_response_from_record(record));
    }
    for values in by_message.values_mut() {
        values.sort_by(|a, b| a.attachment_id.cmp(&b.attachment_id));
    }
    by_message
}

#[cfg(test)]
mod tests {
    use super::{
        attachment_map_from_records, attachment_response_from_record,
        parse_attachment_ids, validate_attachment_filename,
    };
    use crate::server::core::AttachmentRecord;
    use crate::server::core::MAX_ATTACHMENTS_PER_MESSAGE;
    use crate::server::errors::AuthFailure;
    use filament_core::UserId;
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

    #[test]
    fn attachment_response_from_record_maps_expected_fields() {
        let owner_id = UserId::new();
        let record = AttachmentRecord {
            attachment_id: Ulid::new().to_string(),
            guild_id: Ulid::new().to_string(),
            channel_id: Ulid::new().to_string(),
            owner_id,
            filename: String::from("report.png"),
            mime_type: String::from("image/png"),
            size_bytes: 2048,
            sha256_hex: String::from("abc123"),
            object_key: String::from("objects/key"),
            message_id: Some(Ulid::new().to_string()),
        };

        let response = attachment_response_from_record(&record);
        assert_eq!(response.attachment_id, record.attachment_id);
        assert_eq!(response.guild_id, record.guild_id);
        assert_eq!(response.channel_id, record.channel_id);
        assert_eq!(response.owner_id, owner_id.to_string());
        assert_eq!(response.filename, record.filename);
        assert_eq!(response.mime_type, record.mime_type);
        assert_eq!(response.size_bytes, record.size_bytes);
        assert_eq!(response.sha256_hex, record.sha256_hex);
    }

    #[test]
    fn attachment_map_from_records_filters_and_sorts_by_attachment_id() {
        let owner_id = UserId::new();
        let keep_message = Ulid::new().to_string();
        let other_message = Ulid::new().to_string();
        let guild_id = Ulid::new().to_string();
        let channel_id = Ulid::new().to_string();

        let record_a = AttachmentRecord {
            attachment_id: String::from("02ARZ3NDEKTSV4RRFFQ69G5FAV"),
            guild_id: guild_id.clone(),
            channel_id: channel_id.clone(),
            owner_id,
            filename: String::from("b.png"),
            mime_type: String::from("image/png"),
            size_bytes: 2,
            sha256_hex: String::from("b"),
            object_key: String::from("k2"),
            message_id: Some(keep_message.clone()),
        };
        let record_b = AttachmentRecord {
            attachment_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            guild_id: guild_id.clone(),
            channel_id: channel_id.clone(),
            owner_id,
            filename: String::from("a.png"),
            mime_type: String::from("image/png"),
            size_bytes: 1,
            sha256_hex: String::from("a"),
            object_key: String::from("k1"),
            message_id: Some(keep_message.clone()),
        };
        let other_guild = AttachmentRecord {
            attachment_id: Ulid::new().to_string(),
            guild_id: Ulid::new().to_string(),
            channel_id: channel_id.clone(),
            owner_id,
            filename: String::from("skip.png"),
            mime_type: String::from("image/png"),
            size_bytes: 3,
            sha256_hex: String::from("c"),
            object_key: String::from("k3"),
            message_id: Some(keep_message.clone()),
        };
        let other_message_record = AttachmentRecord {
            attachment_id: Ulid::new().to_string(),
            guild_id: guild_id.clone(),
            channel_id: channel_id.clone(),
            owner_id,
            filename: String::from("skip2.png"),
            mime_type: String::from("image/png"),
            size_bytes: 4,
            sha256_hex: String::from("d"),
            object_key: String::from("k4"),
            message_id: Some(other_message.clone()),
        };

        let rows = vec![record_a, record_b, other_guild, other_message_record];
        let map = attachment_map_from_records(
            rows.iter(),
            &guild_id,
            Some(&channel_id),
            std::slice::from_ref(&keep_message),
        );

        assert_eq!(map.len(), 1);
        let kept = map.get(&keep_message).expect("message should be present");
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].attachment_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert_eq!(kept[1].attachment_id, "02ARZ3NDEKTSV4RRFFQ69G5FAV");
    }
}
