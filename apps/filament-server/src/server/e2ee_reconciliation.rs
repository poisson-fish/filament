//! Bounded monitoring for policy-triggered MLS membership reconciliation.

use std::time::Duration;

use sqlx::PgPool;

use super::{
    auth::now_unix, core::AppState, metrics::record_e2ee_membership_reconciliation_observation,
};

const RECONCILIATION_MONITOR_INTERVAL: Duration = Duration::from_secs(30);
const RECONCILIATION_SCAN_LIMIT: i64 = 1_000;
const RECONCILIATION_FETCH_LIMIT: i64 = RECONCILIATION_SCAN_LIMIT + 1;

const PENDING_RECONCILIATIONS_SQL: &str =
    "SELECT deadline_unix FROM e2ee_membership_reconciliations
     WHERE completed_epoch IS NULL
     ORDER BY deadline_unix ASC, reconciliation_id ASC
     LIMIT $1";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ReconciliationObservation {
    pub(crate) pending_sampled: u64,
    pub(crate) overdue_sampled: u64,
    pub(crate) oldest_overdue_seconds: u64,
    pub(crate) scan_saturated: bool,
}

/// Read a bounded oldest-first sample of incomplete membership reconciliations.
pub(crate) async fn observe_e2ee_membership_reconciliations(
    pool: &PgPool,
    current_unix: i64,
) -> Result<ReconciliationObservation, sqlx::Error> {
    let deadlines: Vec<i64> = sqlx::query_scalar(PENDING_RECONCILIATIONS_SQL)
        .bind(RECONCILIATION_FETCH_LIMIT)
        .fetch_all(pool)
        .await?;
    Ok(summarize_deadlines(deadlines, current_unix))
}

fn summarize_deadlines(mut deadlines: Vec<i64>, current_unix: i64) -> ReconciliationObservation {
    let scan_saturated = deadlines.len()
        > usize::try_from(RECONCILIATION_SCAN_LIMIT)
            .expect("positive reconciliation scan limit must fit usize");
    if scan_saturated {
        deadlines.pop();
    }
    let overdue_sampled = deadlines.partition_point(|deadline| *deadline <= current_unix);
    let oldest_overdue_seconds = deadlines
        .first()
        .filter(|deadline| **deadline <= current_unix)
        .and_then(|deadline| current_unix.checked_sub(*deadline))
        .and_then(|seconds| u64::try_from(seconds).ok())
        .unwrap_or(0);
    ReconciliationObservation {
        pending_sampled: u64::try_from(deadlines.len())
            .expect("bounded reconciliation sample length must fit u64"),
        overdue_sampled: u64::try_from(overdue_sampled)
            .expect("bounded overdue sample length must fit u64"),
        oldest_overdue_seconds,
        scan_saturated,
    }
}

/// Monitor pending policy evictions for the lifetime of the server.
pub(crate) async fn start_e2ee_reconciliation_monitor(state: AppState) {
    let Some(pool) = state.db_pool.clone() else {
        return;
    };
    let mut interval = tokio::time::interval(RECONCILIATION_MONITOR_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_overdue = None;
    loop {
        interval.tick().await;
        match observe_e2ee_membership_reconciliations(&pool, now_unix()).await {
            Ok(observation) => {
                record_e2ee_membership_reconciliation_observation(observation);
                let overdue_state = (observation.overdue_sampled, observation.scan_saturated);
                if observation.overdue_sampled > 0 && last_overdue != Some(overdue_state) {
                    tracing::warn!(
                        event = "e2ee.membership_reconciliation.overdue",
                        overdue_sampled = observation.overdue_sampled,
                        oldest_overdue_seconds = observation.oldest_overdue_seconds,
                        scan_saturated = observation.scan_saturated,
                        "policy-triggered MLS removals exceeded the reconciliation deadline"
                    );
                }
                last_overdue = Some(overdue_state);
            }
            Err(error) => tracing::error!(
                event = "e2ee.membership_reconciliation.monitor_failed",
                error = %error,
                "E2EE membership reconciliation monitoring failed"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_query_is_oldest_first_incomplete_and_bounded() {
        assert!(PENDING_RECONCILIATIONS_SQL.contains("completed_epoch IS NULL"));
        assert!(PENDING_RECONCILIATIONS_SQL.contains("ORDER BY deadline_unix ASC"));
        assert!(PENDING_RECONCILIATIONS_SQL.contains("LIMIT $1"));
        assert_eq!(RECONCILIATION_SCAN_LIMIT, 1_000);
        assert_eq!(RECONCILIATION_FETCH_LIMIT, 1_001);
        assert_eq!(RECONCILIATION_MONITOR_INTERVAL, Duration::from_secs(30));
    }

    #[test]
    fn deadline_summary_counts_due_boundary_and_oldest_age() {
        assert_eq!(
            summarize_deadlines(vec![90, 100, 101, 150], 100),
            ReconciliationObservation {
                pending_sampled: 4,
                overdue_sampled: 2,
                oldest_overdue_seconds: 10,
                scan_saturated: false,
            }
        );
    }

    #[test]
    fn deadline_summary_caps_work_and_reports_saturation() {
        let deadlines = (0..RECONCILIATION_FETCH_LIMIT).collect();
        assert_eq!(
            summarize_deadlines(deadlines, i64::MAX),
            ReconciliationObservation {
                pending_sampled: 1_000,
                overdue_sampled: 1_000,
                oldest_overdue_seconds: u64::try_from(i64::MAX)
                    .expect("positive i64 max must fit u64"),
                scan_saturated: true,
            }
        );
    }
}
