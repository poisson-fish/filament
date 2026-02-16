use tokio::sync::{mpsc, oneshot};

use crate::server::{
    core::{SearchCommand, SearchOperation},
    errors::AuthFailure,
};

pub(crate) async fn enqueue_search_command(
    tx: &mpsc::Sender<SearchCommand>,
    op: SearchOperation,
    wait_for_apply: bool,
) -> Result<(), AuthFailure> {
    if wait_for_apply {
        let (ack_tx, ack_rx) = oneshot::channel();
        tx.send(SearchCommand {
            op,
            ack: Some(ack_tx),
        })
        .await
        .map_err(|_| AuthFailure::Internal)?;
        ack_rx.await.map_err(|_| AuthFailure::Internal)?
    } else {
        tx.send(SearchCommand { op, ack: None })
            .await
            .map_err(|_| AuthFailure::Internal)
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::enqueue_search_command;
    use crate::server::{
        core::{SearchCommand, SearchOperation, SEARCH_INDEX_QUEUE_CAPACITY},
        errors::AuthFailure,
    };

    #[tokio::test]
    async fn sends_without_ack_when_wait_is_false() {
        let (tx, mut rx) = mpsc::channel::<SearchCommand>(SEARCH_INDEX_QUEUE_CAPACITY);

        enqueue_search_command(
            &tx,
            SearchOperation::Delete {
                message_id: String::from("m1"),
            },
            false,
        )
        .await
        .expect("enqueue should succeed");

        let command = rx.recv().await.expect("command should be queued");
        assert!(command.ack.is_none());
        match command.op {
            SearchOperation::Delete { message_id } => assert_eq!(message_id, "m1"),
            _ => panic!("expected delete operation"),
        }
    }

    #[tokio::test]
    async fn waits_for_ack_when_wait_is_true() {
        let (tx, mut rx) = mpsc::channel::<SearchCommand>(SEARCH_INDEX_QUEUE_CAPACITY);
        let receive_task = tokio::spawn(async move {
            let command = rx.recv().await.expect("command should be queued");
            let ack = command.ack.expect("ack channel should be present");
            ack.send(Ok(())).expect("ack should be delivered");
        });

        enqueue_search_command(
            &tx,
            SearchOperation::Delete {
                message_id: String::from("m2"),
            },
            true,
        )
        .await
        .expect("enqueue should succeed with ack");

        receive_task.await.expect("receiver task should join");
    }

    #[tokio::test]
    async fn returns_internal_when_ack_channel_closes_without_response() {
        let (tx, mut rx) = mpsc::channel::<SearchCommand>(SEARCH_INDEX_QUEUE_CAPACITY);
        let receive_task = tokio::spawn(async move {
            let _command = rx.recv().await.expect("command should be queued");
        });

        let result = enqueue_search_command(
            &tx,
            SearchOperation::Delete {
                message_id: String::from("m3"),
            },
            true,
        )
        .await;

        assert!(matches!(result, Err(AuthFailure::Internal)));
        receive_task.await.expect("receiver task should join");
    }

    #[tokio::test]
    async fn returns_internal_when_sender_channel_is_closed() {
        let (tx, rx) = mpsc::channel::<SearchCommand>(SEARCH_INDEX_QUEUE_CAPACITY);
        drop(rx);

        let result = enqueue_search_command(
            &tx,
            SearchOperation::Delete {
                message_id: String::from("m4"),
            },
            false,
        )
        .await;

        assert!(matches!(result, Err(AuthFailure::Internal)));
    }
}