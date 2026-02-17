use ulid::Ulid;

use crate::server::{
    core::{AttachmentRecord, MAX_ATTACHMENTS_PER_MESSAGE},
    errors::AuthFailure,
    types::{AttachmentResponse, MessageResponse},
};
use filament_core::UserId;
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

pub(crate) fn attachment_response_from_db_fields(
    attachment_id: String,
    guild_id: String,
    channel_id: String,
    owner_id: String,
    filename: String,
    mime_type: String,
    size_bytes: i64,
    sha256_hex: String,
) -> Result<AttachmentResponse, AuthFailure> {
    Ok(AttachmentResponse {
        attachment_id,
        guild_id,
        channel_id,
        owner_id,
        filename,
        mime_type,
        size_bytes: u64::try_from(size_bytes).map_err(|_| AuthFailure::Internal)?,
        sha256_hex,
    })
}

pub(crate) fn attachment_record_from_db_fields(
    attachment_id: String,
    guild_id: String,
    channel_id: String,
    owner_id: String,
    filename: String,
    mime_type: String,
    size_bytes: i64,
    sha256_hex: String,
    object_key: String,
    message_id: Option<String>,
) -> Result<AttachmentRecord, AuthFailure> {
    Ok(AttachmentRecord {
        attachment_id,
        guild_id,
        channel_id,
        owner_id: UserId::try_from(owner_id).map_err(|_| AuthFailure::Internal)?,
        filename,
        mime_type,
        size_bytes: u64::try_from(size_bytes).map_err(|_| AuthFailure::Internal)?,
        sha256_hex,
        object_key,
        message_id,
    })
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

pub(crate) fn attachment_map_from_db_records(
    records: Vec<(Option<String>, AttachmentResponse)>,
) -> HashMap<String, Vec<AttachmentResponse>> {
    let mut by_message: HashMap<String, Vec<AttachmentResponse>> = HashMap::new();
    for (message_id, response) in records {
        let Some(message_id) = message_id else {
            continue;
        };
        by_message.entry(message_id).or_default().push(response);
    }
    by_message
}

pub(crate) fn attachments_from_ids_in_memory(
    attachments: &HashMap<String, AttachmentRecord>,
    attachment_ids: &[String],
) -> Result<Vec<AttachmentResponse>, AuthFailure> {
    if attachment_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::with_capacity(attachment_ids.len());
    for attachment_id in attachment_ids {
        let Some(record) = attachments.get(attachment_id) else {
            return Err(AuthFailure::InvalidRequest);
        };
        out.push(attachment_response_from_record(record));
    }
    Ok(out)
}

pub(crate) fn attachment_usage_for_owner<'a>(
    records: impl Iterator<Item = &'a AttachmentRecord>,
    owner_id: UserId,
) -> u64 {
    records
        .filter(|record| record.owner_id == owner_id)
        .map(|record| record.size_bytes)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::{
        attachment_map_from_db_records, attachment_map_from_records,
        attachment_record_from_db_fields,
        attachment_response_from_db_fields,
        attachment_response_from_record, attachment_usage_for_owner,
        attachments_from_ids_in_memory, parse_attachment_ids,
        validate_attachment_filename,
    };
    use crate::server::core::AttachmentRecord;
    use crate::server::core::MAX_ATTACHMENTS_PER_MESSAGE;
    use crate::server::errors::AuthFailure;
    use crate::server::types::AttachmentResponse;
    use filament_core::UserId;
    use std::collections::HashMap;
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
    fn attachment_response_from_db_fields_maps_expected_fields() {
        let response = attachment_response_from_db_fields(
            String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            String::from("guild-1"),
            String::from("channel-1"),
            String::from("01ARZ3NDEKTSV4RRFFQ69G5FBB"),
            String::from("report.png"),
            String::from("image/png"),
            2048,
            String::from("abc123"),
        )
        .expect("db fields should map to attachment response");
        assert_eq!(response.attachment_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert_eq!(response.guild_id, "guild-1");
        assert_eq!(response.channel_id, "channel-1");
        assert_eq!(response.owner_id, "01ARZ3NDEKTSV4RRFFQ69G5FBB");
        assert_eq!(response.filename, "report.png");
        assert_eq!(response.mime_type, "image/png");
        assert_eq!(response.size_bytes, 2048);
        assert_eq!(response.sha256_hex, "abc123");
    }

    #[test]
    fn attachment_record_from_db_fields_maps_expected_fields() {
        let record = attachment_record_from_db_fields(
            String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            String::from("guild-1"),
            String::from("channel-1"),
            String::from("01ARZ3NDEKTSV4RRFFQ69G5FBB"),
            String::from("report.png"),
            String::from("image/png"),
            2048,
            String::from("abc123"),
            String::from("objects/key"),
            Some(String::from("01ARZ3NDEKTSV4RRFFQ69G5FCC")),
        )
        .expect("db fields should map to attachment record");
        assert_eq!(record.attachment_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert_eq!(record.guild_id, "guild-1");
        assert_eq!(record.channel_id, "channel-1");
        assert_eq!(record.owner_id.to_string(), "01ARZ3NDEKTSV4RRFFQ69G5FBB");
        assert_eq!(record.filename, "report.png");
        assert_eq!(record.mime_type, "image/png");
        assert_eq!(record.size_bytes, 2048);
        assert_eq!(record.sha256_hex, "abc123");
        assert_eq!(record.object_key, "objects/key");
        assert_eq!(record.message_id.as_deref(), Some("01ARZ3NDEKTSV4RRFFQ69G5FCC"));
    }

    #[test]
    fn attachment_response_from_db_fields_rejects_negative_size_fail_closed() {
        assert!(matches!(
            attachment_response_from_db_fields(
                String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
                String::from("guild-1"),
                String::from("channel-1"),
                String::from("01ARZ3NDEKTSV4RRFFQ69G5FBB"),
                String::from("report.png"),
                String::from("image/png"),
                -1,
                String::from("abc123"),
            ),
            Err(AuthFailure::Internal)
        ));
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

    #[test]
    fn attachment_map_from_db_records_groups_by_message_and_skips_null_message_id() {
        let entry_a = AttachmentResponse {
            attachment_id: String::from("att-a"),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            owner_id: UserId::new().to_string(),
            filename: String::from("a.png"),
            mime_type: String::from("image/png"),
            size_bytes: 1,
            sha256_hex: String::from("hash-a"),
        };
        let entry_b = AttachmentResponse {
            attachment_id: String::from("att-b"),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            owner_id: UserId::new().to_string(),
            filename: String::from("b.png"),
            mime_type: String::from("image/png"),
            size_bytes: 2,
            sha256_hex: String::from("hash-b"),
        };

        let map = attachment_map_from_db_records(vec![
            (Some(String::from("m1")), entry_a.clone()),
            (None, entry_b.clone()),
            (Some(String::from("m1")), entry_b),
        ]);

        assert_eq!(map.len(), 1);
        let grouped = map.get("m1").expect("m1 should be present");
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].attachment_id, entry_a.attachment_id);
        assert_eq!(grouped[1].attachment_id, "att-b");
    }

    #[test]
    fn attachments_from_ids_in_memory_returns_ordered_responses() {
        let owner_id = UserId::new();
        let first_id = Ulid::new().to_string();
        let second_id = Ulid::new().to_string();

        let mut attachments = HashMap::new();
        attachments.insert(
            first_id.clone(),
            AttachmentRecord {
                attachment_id: first_id.clone(),
                guild_id: String::from("g1"),
                channel_id: String::from("c1"),
                owner_id,
                filename: String::from("a.png"),
                mime_type: String::from("image/png"),
                size_bytes: 10,
                sha256_hex: String::from("hash-a"),
                object_key: String::from("obj-a"),
                message_id: None,
            },
        );
        attachments.insert(
            second_id.clone(),
            AttachmentRecord {
                attachment_id: second_id.clone(),
                guild_id: String::from("g1"),
                channel_id: String::from("c1"),
                owner_id,
                filename: String::from("b.png"),
                mime_type: String::from("image/png"),
                size_bytes: 20,
                sha256_hex: String::from("hash-b"),
                object_key: String::from("obj-b"),
                message_id: None,
            },
        );

        let responses = attachments_from_ids_in_memory(
            &attachments,
            &[second_id.clone(), first_id.clone()],
        )
        .expect("attachment IDs should resolve");

        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].attachment_id, second_id);
        assert_eq!(responses[1].attachment_id, first_id);
    }

    #[test]
    fn attachments_from_ids_in_memory_rejects_missing_id_fail_closed() {
        let attachments = HashMap::new();
        assert!(matches!(
            attachments_from_ids_in_memory(&attachments, &[Ulid::new().to_string()]),
            Err(AuthFailure::InvalidRequest)
        ));
    }

    #[test]
    fn attachment_usage_for_owner_sums_only_matching_owner_records() {
        let owner_id = UserId::new();
        let other_owner = UserId::new();
        let records = vec![
            AttachmentRecord {
                attachment_id: Ulid::new().to_string(),
                guild_id: String::from("g1"),
                channel_id: String::from("c1"),
                owner_id,
                filename: String::from("a.png"),
                mime_type: String::from("image/png"),
                size_bytes: 10,
                sha256_hex: String::from("ha"),
                object_key: String::from("oa"),
                message_id: None,
            },
            AttachmentRecord {
                attachment_id: Ulid::new().to_string(),
                guild_id: String::from("g1"),
                channel_id: String::from("c1"),
                owner_id,
                filename: String::from("b.png"),
                mime_type: String::from("image/png"),
                size_bytes: 15,
                sha256_hex: String::from("hb"),
                object_key: String::from("ob"),
                message_id: None,
            },
            AttachmentRecord {
                attachment_id: Ulid::new().to_string(),
                guild_id: String::from("g1"),
                channel_id: String::from("c1"),
                owner_id: other_owner,
                filename: String::from("c.png"),
                mime_type: String::from("image/png"),
                size_bytes: 99,
                sha256_hex: String::from("hc"),
                object_key: String::from("oc"),
                message_id: None,
            },
        ];

        let usage = attachment_usage_for_owner(records.iter(), owner_id);
        assert_eq!(usage, 25);
    }

    #[test]
    fn attachment_usage_for_owner_returns_zero_for_no_matches() {
        let usage = attachment_usage_for_owner([].iter(), UserId::new());
        assert_eq!(usage, 0);
    }
}
