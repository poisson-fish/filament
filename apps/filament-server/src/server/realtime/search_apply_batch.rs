use std::sync::Arc;

use crate::server::{
    core::{SearchCommand, SearchIndexState, SearchOperation},
    errors::AuthFailure,
};

pub(crate) fn apply_search_batch_with_ack<F>(
    search: &Arc<SearchIndexState>,
    batch: &mut Vec<SearchCommand>,
    apply_op: F,
) -> anyhow::Result<()>
where
    F: Fn(&SearchIndexState, &mut tantivy::IndexWriter, SearchOperation),
{
    let mut ops = Vec::with_capacity(batch.len());
    let mut pending_acks = Vec::new();
    for command in batch.drain(..) {
        if let Some(ack) = command.ack {
            pending_acks.push(ack);
        }
        ops.push(command.op);
    }

    let apply_result = (|| -> anyhow::Result<()> {
        let mut writer = search.index.writer(50_000_000)?;
        for op in ops {
            apply_op(search, &mut writer, op);
        }
        writer.commit()?;
        search.reader.reload()?;
        Ok(())
    })();

    match apply_result {
        Ok(()) => {
            for ack in pending_acks {
                let _ = ack.send(Ok(()));
            }
            Ok(())
        }
        Err(error) => {
            for ack in pending_acks {
                let _ = ack.send(Err(AuthFailure::Internal));
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::oneshot;

    use super::apply_search_batch_with_ack;
    use crate::server::{
        core::{SearchCommand, SearchIndexState, SearchOperation},
        errors::AuthFailure,
        realtime::search_schema::build_search_schema,
    };

    fn search_state() -> Arc<SearchIndexState> {
        let (schema, fields) = build_search_schema();
        let index = tantivy::Index::create_in_ram(schema);
        let reader = index.reader().expect("reader should initialize");
        Arc::new(SearchIndexState {
            index,
            reader,
            fields,
        })
    }

    #[test]
    fn sends_success_ack_when_batch_applies() {
        let search = search_state();
        let (ack_tx, ack_rx) = oneshot::channel();
        let mut batch = vec![SearchCommand {
            op: SearchOperation::Delete {
                message_id: String::from("m1"),
            },
            ack: Some(ack_tx),
        }];

        let result = apply_search_batch_with_ack(&search, &mut batch, |_search, _writer, _op| {});

        assert!(result.is_ok());
        assert!(batch.is_empty());
        assert!(matches!(ack_rx.blocking_recv(), Ok(Ok(()))));
    }

    #[test]
    fn sends_internal_ack_when_batch_apply_fails() {
        let search = search_state();
        let writer_guard: tantivy::IndexWriter<tantivy::schema::TantivyDocument> = search
            .index
            .writer(50_000_000)
            .expect("lock writer for failure path");
        let (ack_tx, ack_rx) = oneshot::channel();
        let mut batch = vec![SearchCommand {
            op: SearchOperation::Delete {
                message_id: String::from("m2"),
            },
            ack: Some(ack_tx),
        }];

        let result = apply_search_batch_with_ack(&search, &mut batch, |_search, _writer, _op| {});

        assert!(result.is_err());
        assert!(batch.is_empty());
        assert!(matches!(
            ack_rx.blocking_recv(),
            Ok(Err(AuthFailure::Internal))
        ));
        drop(writer_guard);
    }
}
