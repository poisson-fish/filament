use tokio::sync::mpsc;

use crate::server::core::SearchCommand;

pub(crate) fn drain_search_batch(
    first: SearchCommand,
    rx: &mut mpsc::Receiver<SearchCommand>,
    max_batch: usize,
) -> Vec<SearchCommand> {
    let max_batch = max_batch.max(1);
    let mut batch = vec![first];
    while batch.len() < max_batch {
        let Ok(next) = rx.try_recv() else {
            break;
        };
        batch.push(next);
    }
    batch
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::drain_search_batch;
    use crate::server::core::{SearchCommand, SearchOperation};

    fn command(message_id: &str) -> SearchCommand {
        SearchCommand {
            op: SearchOperation::Delete {
                message_id: message_id.to_owned(),
            },
            ack: None,
        }
    }

    fn message_id(command: &SearchCommand) -> Option<&str> {
        match &command.op {
            SearchOperation::Delete { message_id } => Some(message_id.as_str()),
            _ => None,
        }
    }

    #[test]
    fn drains_up_to_max_batch_size() {
        let (tx, mut rx) = mpsc::channel::<SearchCommand>(8);
        tx.try_send(command("m2"))
            .expect("second command should queue");
        tx.try_send(command("m3"))
            .expect("third command should queue");

        let batch = drain_search_batch(command("m1"), &mut rx, 2);

        assert_eq!(batch.len(), 2);
        assert_eq!(message_id(&batch[0]), Some("m1"));
        assert_eq!(message_id(&batch[1]), Some("m2"));
        assert_eq!(rx.try_recv().ok().as_ref().and_then(message_id), Some("m3"));
    }

    #[test]
    fn defaults_to_single_item_when_max_batch_is_zero() {
        let (tx, mut rx) = mpsc::channel::<SearchCommand>(4);
        tx.try_send(command("m2"))
            .expect("second command should queue");

        let batch = drain_search_batch(command("m1"), &mut rx, 0);

        assert_eq!(batch.len(), 1);
        assert_eq!(message_id(&batch[0]), Some("m1"));
        assert_eq!(rx.try_recv().ok().as_ref().and_then(message_id), Some("m2"));
    }
}