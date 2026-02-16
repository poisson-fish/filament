use std::time::Duration;

use crate::server::errors::AuthFailure;

pub(crate) async fn run_search_blocking_with_timeout<T, F>(
    timeout: Duration,
    task: F,
) -> Result<T, AuthFailure>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AuthFailure> + Send + 'static,
{
    tokio::time::timeout(timeout, async move {
        tokio::task::spawn_blocking(task)
            .await
            .map_err(|_| AuthFailure::Internal)?
    })
    .await
    .map_err(|_| AuthFailure::InvalidRequest)?
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::run_search_blocking_with_timeout;
    use crate::server::errors::AuthFailure;

    #[tokio::test]
    async fn returns_task_result_before_timeout() {
        let result = run_search_blocking_with_timeout(Duration::from_millis(100), || Ok(42_i32))
            .await
            .expect("task should complete");

        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn fails_closed_when_timeout_expires() {
        let result = run_search_blocking_with_timeout(Duration::from_millis(20), || {
            thread::sleep(Duration::from_millis(80));
            Ok(1_i32)
        })
        .await;

        assert!(matches!(result, Err(AuthFailure::InvalidRequest)));
    }

    #[tokio::test]
    async fn maps_task_panic_to_internal_error() {
        let result: Result<i32, AuthFailure> =
            run_search_blocking_with_timeout(Duration::from_millis(100), || {
                panic!("simulated panic")
            })
            .await;

        assert!(matches!(result, Err(AuthFailure::Internal)));
    }
}
