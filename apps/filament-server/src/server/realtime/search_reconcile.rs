use std::collections::HashSet;

use crate::server::core::IndexedMessage;

pub(crate) fn compute_reconciliation(
    source_docs: Vec<IndexedMessage>,
    index_ids: HashSet<String>,
) -> (Vec<IndexedMessage>, Vec<String>) {
    let source_ids: HashSet<String> = source_docs
        .iter()
        .map(|doc| doc.message_id.clone())
        .collect();
    let mut upserts: Vec<IndexedMessage> = source_docs
        .into_iter()
        .filter(|doc| !index_ids.contains(&doc.message_id))
        .collect();
    let mut delete_message_ids: Vec<String> = index_ids
        .into_iter()
        .filter(|message_id| !source_ids.contains(message_id))
        .collect();
    upserts.sort_by(|a, b| a.message_id.cmp(&b.message_id));
    delete_message_ids.sort_unstable();
    (upserts, delete_message_ids)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::compute_reconciliation;
    use crate::server::core::IndexedMessage;

    fn doc(id: &str) -> IndexedMessage {
        IndexedMessage {
            message_id: id.to_owned(),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            author_id: String::from("u1"),
            created_at_unix: 1,
            content: format!("content-{id}"),
        }
    }

    #[test]
    fn computes_upserts_and_deletes_and_sorts_results() {
        let source_docs = vec![doc("m3"), doc("m1"), doc("m2")];
        let index_ids = HashSet::from([String::from("m2"), String::from("m4")]);

        let (upserts, deletes) = compute_reconciliation(source_docs, index_ids);

        let upsert_ids: Vec<String> = upserts.into_iter().map(|entry| entry.message_id).collect();
        assert_eq!(upsert_ids, vec![String::from("m1"), String::from("m3")]);
        assert_eq!(deletes, vec![String::from("m4")]);
    }

    #[test]
    fn returns_empty_sets_when_source_and_index_match() {
        let source_docs = vec![doc("m1"), doc("m2")];
        let index_ids = HashSet::from([String::from("m1"), String::from("m2")]);

        let (upserts, deletes) = compute_reconciliation(source_docs, index_ids);

        assert!(upserts.is_empty());
        assert!(deletes.is_empty());
    }
}
