use sqlx::{Postgres, Transaction};

const CREATE_E2EE_CONVERSATIONS_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_conversations (
    conversation_id TEXT PRIMARY KEY,
    conversation_crypto TEXT NOT NULL CHECK (conversation_crypto IN ('plaintext', 'mls_v1')),
    created_by       TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    created_at_unix  BIGINT NOT NULL
)";

const CREATE_E2EE_CONVERSATION_MEMBERS_SQL: &str =
    "CREATE TABLE IF NOT EXISTS e2ee_conversation_members (
    conversation_id TEXT NOT NULL REFERENCES e2ee_conversations(conversation_id) ON DELETE CASCADE,
    user_id         TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    joined_at_unix  BIGINT NOT NULL,
    PRIMARY KEY (conversation_id, user_id)
)";

const CREATE_E2EE_GROUPS_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_groups (
    group_id          TEXT PRIMARY KEY,
    conversation_id   TEXT NOT NULL UNIQUE REFERENCES e2ee_conversations(conversation_id) ON DELETE CASCADE,
    crypto_mode       TEXT NOT NULL DEFAULT 'mls_v1' CHECK (crypto_mode = 'mls_v1'),
    current_epoch     BIGINT NOT NULL CHECK (current_epoch >= 0),
    suite_id          INTEGER NOT NULL CHECK (suite_id IN (1, 2, 3, 4, 5, 6, 7)),
    group_info_blob   BYTEA CHECK (
        group_info_blob IS NULL OR
        (octet_length(group_info_blob) > 0 AND octet_length(group_info_blob) <= 65536)
    ),
    created_at_unix   BIGINT NOT NULL
)";

const CREATE_E2EE_COMMITS_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_commits (
    group_id            TEXT NOT NULL REFERENCES e2ee_groups(group_id) ON DELETE CASCADE,
    epoch               BIGINT NOT NULL CHECK (epoch > 0),
    prior_epoch         BIGINT NOT NULL CHECK (prior_epoch >= 0 AND epoch = prior_epoch + 1),
    committer_device_id TEXT NOT NULL REFERENCES e2ee_device_certificates(device_id),
    commit_blob         BYTEA NOT NULL CHECK (
        octet_length(commit_blob) > 0 AND octet_length(commit_blob) <= 65536
    ),
    welcome_blob        BYTEA CHECK (
        welcome_blob IS NULL OR
        (octet_length(welcome_blob) > 0 AND octet_length(welcome_blob) <= 65536)
    ),
    created_at_unix     BIGINT NOT NULL,
    expires_at_unix     BIGINT NOT NULL CHECK (expires_at_unix > created_at_unix),
    PRIMARY KEY (group_id, epoch)
)";

const CREATE_E2EE_MESSAGES_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_messages (
    message_id          TEXT PRIMARY KEY,
    group_id            TEXT NOT NULL REFERENCES e2ee_groups(group_id) ON DELETE CASCADE,
    sender_device_id    TEXT NOT NULL REFERENCES e2ee_device_certificates(device_id),
    epoch               BIGINT NOT NULL CHECK (epoch >= 0),
    suite_id            INTEGER NOT NULL CHECK (suite_id IN (1, 2, 3, 4, 5, 6, 7)),
    crypto_mode         TEXT NOT NULL DEFAULT 'mls_v1' CHECK (crypto_mode = 'mls_v1'),
    ciphertext_blob     BYTEA NOT NULL CHECK (
        octet_length(ciphertext_blob) IN (512, 1024, 4096, 16384)
    ),
    created_at_unix     BIGINT NOT NULL,
    expires_at_unix     BIGINT NOT NULL CHECK (expires_at_unix > created_at_unix)
)";

const CREATE_E2EE_MESSAGE_ACKS_SQL: &str = "CREATE TABLE IF NOT EXISTS e2ee_message_acks (
    message_id     TEXT NOT NULL REFERENCES e2ee_messages(message_id) ON DELETE CASCADE,
    device_id      TEXT NOT NULL REFERENCES e2ee_device_certificates(device_id),
    acked_at_unix  BIGINT,
    PRIMARY KEY (message_id, device_id)
)";

const INDEX_E2EE_CONVERSATION_MEMBERS_BY_USER_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_e2ee_conversation_members_user
     ON e2ee_conversation_members (user_id, conversation_id)";

const INDEX_E2EE_MESSAGES_MAILBOX_SQL: &str = "CREATE INDEX IF NOT EXISTS idx_e2ee_messages_mailbox
     ON e2ee_messages (group_id, message_id)";

const INDEX_E2EE_MESSAGES_EXPIRY_SQL: &str = "CREATE INDEX IF NOT EXISTS idx_e2ee_messages_expiry
     ON e2ee_messages (expires_at_unix)";

/// Apply the v13 E2EE Delivery Service persistence schema.
///
/// MLS interiors remain opaque. Database constraints repeat all wire-level
/// bounds so a future write path cannot bypass the HTTP boundary checks.
pub(crate) async fn apply_e2ee_message_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    for statement in [
        CREATE_E2EE_CONVERSATIONS_SQL,
        CREATE_E2EE_CONVERSATION_MEMBERS_SQL,
        CREATE_E2EE_GROUPS_SQL,
        CREATE_E2EE_COMMITS_SQL,
        CREATE_E2EE_MESSAGES_SQL,
        CREATE_E2EE_MESSAGE_ACKS_SQL,
        INDEX_E2EE_CONVERSATION_MEMBERS_BY_USER_SQL,
        INDEX_E2EE_MESSAGES_MAILBOX_SQL,
        INDEX_E2EE_MESSAGES_EXPIRY_SQL,
    ] {
        sqlx::query(statement).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_mode_and_group_mapping_are_database_invariants() {
        assert!(CREATE_E2EE_CONVERSATIONS_SQL
            .contains("conversation_crypto IN ('plaintext', 'mls_v1')"));
        assert!(CREATE_E2EE_GROUPS_SQL.contains("conversation_id   TEXT NOT NULL UNIQUE"));
        assert!(CREATE_E2EE_GROUPS_SQL.contains("crypto_mode = 'mls_v1'"));
        assert!(CREATE_E2EE_GROUPS_SQL.contains("current_epoch >= 0"));
        assert!(CREATE_E2EE_GROUPS_SQL.contains("octet_length(group_info_blob) <= 65536"));
    }

    #[test]
    fn commit_schema_enforces_single_writer_epoch_shape_and_blob_caps() {
        assert!(CREATE_E2EE_COMMITS_SQL.contains("PRIMARY KEY (group_id, epoch)"));
        assert!(CREATE_E2EE_COMMITS_SQL.contains("epoch = prior_epoch + 1"));
        assert!(CREATE_E2EE_COMMITS_SQL.contains("octet_length(commit_blob) <= 65536"));
        assert!(CREATE_E2EE_COMMITS_SQL.contains("octet_length(welcome_blob) <= 65536"));
        assert!(CREATE_E2EE_COMMITS_SQL.contains("expires_at_unix > created_at_unix"));
    }

    #[test]
    fn message_schema_is_opaque_padded_and_ttl_bounded() {
        assert!(CREATE_E2EE_MESSAGES_SQL.contains("crypto_mode = 'mls_v1'"));
        assert!(CREATE_E2EE_MESSAGES_SQL
            .contains("octet_length(ciphertext_blob) IN (512, 1024, 4096, 16384)"));
        assert!(CREATE_E2EE_MESSAGES_SQL.contains("expires_at_unix > created_at_unix"));
        assert!(!CREATE_E2EE_MESSAGES_SQL.contains("content TEXT"));
        assert!(CREATE_E2EE_MESSAGE_ACKS_SQL.contains("ON DELETE CASCADE"));
        assert!(CREATE_E2EE_MESSAGE_ACKS_SQL.contains("acked_at_unix  BIGINT,"));
    }
}
