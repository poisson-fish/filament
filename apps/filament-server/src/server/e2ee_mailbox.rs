//! Bounded garbage collection for transient E2EE Delivery Service records.

use sqlx::PgPool;

use super::{auth::now_unix, core::AppState};

const MAILBOX_GC_BATCH_SIZE: i64 = 1_000;
const MAILBOX_GC_MAX_BATCHES_PER_TICK: usize = 10;

const DELETE_EXPIRED_MESSAGES_SQL: &str = "DELETE FROM e2ee_messages
     WHERE message_id IN (
         SELECT message_id FROM e2ee_messages
         WHERE expires_at_unix <= $1
         ORDER BY expires_at_unix ASC, message_id ASC
         LIMIT $2
     )";

const DELETE_EXPIRED_COMMITS_SQL: &str = "DELETE FROM e2ee_commits
     WHERE (group_id, epoch) IN (
         SELECT group_id, epoch FROM e2ee_commits
         WHERE expires_at_unix <= $1
         ORDER BY expires_at_unix ASC, group_id ASC, epoch ASC
         LIMIT $2
     )";

const DELETE_EXPIRED_PROPOSALS_SQL: &str = "DELETE FROM e2ee_proposals
     WHERE proposal_id IN (
         SELECT proposal_id FROM e2ee_proposals
         WHERE expires_at_unix <= $1
         ORDER BY expires_at_unix ASC, proposal_id ASC
         LIMIT $2
     )";

const DELETE_EXPIRED_ATTACHMENTS_SQL: &str = "DELETE FROM e2ee_attachment_blobs
     WHERE attachment_id IN (
         SELECT attachment_id FROM e2ee_attachment_blobs
         WHERE expires_at_unix <= $1
         ORDER BY expires_at_unix ASC, attachment_id ASC
         LIMIT $2
     )";

const DELETE_EXPIRED_COMMIT_RECEIPTS_SQL: &str = "DELETE FROM e2ee_commit_receipts
     WHERE (group_id, epoch) IN (
         SELECT group_id, epoch FROM e2ee_commit_receipts
         WHERE expires_at_unix <= $1
         ORDER BY expires_at_unix ASC, group_id ASC, epoch ASC
         LIMIT $2
     )";

const DELETE_EXPIRED_MESSAGE_RECEIPTS_SQL: &str = "DELETE FROM e2ee_message_receipts
     WHERE (group_id, sender_device_id, ciphertext_sha256) IN (
         SELECT group_id, sender_device_id, ciphertext_sha256
         FROM e2ee_message_receipts
         WHERE expires_at_unix <= $1
         ORDER BY expires_at_unix ASC, group_id ASC, sender_device_id ASC
         LIMIT $2
     )";

/// Hard-delete expired transient E2EE records in bounded batches.
pub(crate) async fn purge_expired_e2ee_mailbox(
    pool: &PgPool,
    current_unix: i64,
) -> Result<(u64, u64, u64, u64, u64, u64), sqlx::Error> {
    let mut deleted_messages = 0_u64;
    let mut deleted_commits = 0_u64;
    let mut deleted_proposals = 0_u64;
    let mut deleted_attachments = 0_u64;
    let mut deleted_commit_receipts = 0_u64;
    let mut deleted_message_receipts = 0_u64;
    for _ in 0..MAILBOX_GC_MAX_BATCHES_PER_TICK {
        let mut transaction = pool.begin().await?;
        let messages = sqlx::query(DELETE_EXPIRED_MESSAGES_SQL)
            .bind(current_unix)
            .bind(MAILBOX_GC_BATCH_SIZE)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
        let commits = sqlx::query(DELETE_EXPIRED_COMMITS_SQL)
            .bind(current_unix)
            .bind(MAILBOX_GC_BATCH_SIZE)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
        let proposals = sqlx::query(DELETE_EXPIRED_PROPOSALS_SQL)
            .bind(current_unix)
            .bind(MAILBOX_GC_BATCH_SIZE)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
        let attachments = sqlx::query(DELETE_EXPIRED_ATTACHMENTS_SQL)
            .bind(current_unix)
            .bind(MAILBOX_GC_BATCH_SIZE)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
        let commit_receipts = sqlx::query(DELETE_EXPIRED_COMMIT_RECEIPTS_SQL)
            .bind(current_unix)
            .bind(MAILBOX_GC_BATCH_SIZE)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
        let message_receipts = sqlx::query(DELETE_EXPIRED_MESSAGE_RECEIPTS_SQL)
            .bind(current_unix)
            .bind(MAILBOX_GC_BATCH_SIZE)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
        transaction.commit().await?;
        deleted_messages = deleted_messages.saturating_add(messages);
        deleted_commits = deleted_commits.saturating_add(commits);
        deleted_proposals = deleted_proposals.saturating_add(proposals);
        deleted_attachments = deleted_attachments.saturating_add(attachments);
        deleted_commit_receipts = deleted_commit_receipts.saturating_add(commit_receipts);
        deleted_message_receipts = deleted_message_receipts.saturating_add(message_receipts);
        if messages < MAILBOX_GC_BATCH_SIZE as u64
            && commits < MAILBOX_GC_BATCH_SIZE as u64
            && proposals < MAILBOX_GC_BATCH_SIZE as u64
            && attachments < MAILBOX_GC_BATCH_SIZE as u64
            && commit_receipts < MAILBOX_GC_BATCH_SIZE as u64
            && message_receipts < MAILBOX_GC_BATCH_SIZE as u64
        {
            break;
        }
    }
    Ok((
        deleted_messages,
        deleted_commits,
        deleted_proposals,
        deleted_attachments,
        deleted_commit_receipts,
        deleted_message_receipts,
    ))
}

/// Run the mailbox TTL sweeper for the lifetime of the server.
pub(crate) async fn start_e2ee_mailbox_gc(state: AppState) {
    let Some(pool) = state.db_pool.clone() else {
        return;
    };
    let mut interval = tokio::time::interval(state.runtime.e2ee_mailbox_gc_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        match purge_expired_e2ee_mailbox(&pool, now_unix()).await {
            Ok((0, 0, 0, 0, 0, 0)) => {}
            Ok((messages, commits, proposals, attachments, commit_receipts, message_receipts)) => {
                tracing::info!(
                    event = "e2ee.mailbox.gc",
                    deleted_messages = messages,
                    deleted_commits = commits,
                    deleted_proposals = proposals,
                    deleted_attachments = attachments,
                    deleted_commit_receipts = commit_receipts,
                    deleted_message_receipts = message_receipts,
                    "hard-deleted expired E2EE mailbox records"
                );
            }
            Err(error) => tracing::error!(
                event = "e2ee.mailbox.gc_failed",
                error = %error,
                "E2EE mailbox garbage collection failed"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gc_queries_are_expiry_scoped_and_bounded() {
        assert!(DELETE_EXPIRED_MESSAGES_SQL.contains("expires_at_unix <= $1"));
        assert!(DELETE_EXPIRED_MESSAGES_SQL.contains("LIMIT $2"));
        assert!(DELETE_EXPIRED_COMMITS_SQL.contains("expires_at_unix <= $1"));
        assert!(DELETE_EXPIRED_COMMITS_SQL.contains("LIMIT $2"));
        assert!(DELETE_EXPIRED_PROPOSALS_SQL.contains("expires_at_unix <= $1"));
        assert!(DELETE_EXPIRED_PROPOSALS_SQL.contains("LIMIT $2"));
        assert!(DELETE_EXPIRED_ATTACHMENTS_SQL.contains("expires_at_unix <= $1"));
        assert!(DELETE_EXPIRED_ATTACHMENTS_SQL.contains("LIMIT $2"));
        assert!(DELETE_EXPIRED_COMMIT_RECEIPTS_SQL.contains("expires_at_unix <= $1"));
        assert!(DELETE_EXPIRED_COMMIT_RECEIPTS_SQL.contains("LIMIT $2"));
        assert!(DELETE_EXPIRED_MESSAGE_RECEIPTS_SQL.contains("expires_at_unix <= $1"));
        assert!(DELETE_EXPIRED_MESSAGE_RECEIPTS_SQL.contains("LIMIT $2"));
        assert_eq!(MAILBOX_GC_BATCH_SIZE, 1_000);
        assert_eq!(MAILBOX_GC_MAX_BATCHES_PER_TICK, 10);
    }
}
