use crate::server::core::{IndexedMessage, SearchOperation};

pub(crate) fn build_search_rebuild_operation(
    docs: Vec<IndexedMessage>,
) -> SearchOperation {
    SearchOperation::Rebuild { docs }
}

#[cfg(test)]
mod tests {
    use super::build_search_rebuild_operation;
    use crate::server::core::{IndexedMessage, SearchOperation};

    fn sample_doc(id: &str) -> IndexedMessage {
        IndexedMessage {
            message_id: id.to_owned(),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            author_id: String::from("u1"),
            created_at_unix: 1,
            content: String::from("hello"),
        }
    }

    #[test]
    fn wraps_docs_in_rebuild_operation() {
        let op = build_search_rebuild_operation(vec![sample_doc("m1"), sample_doc("m2")]);

        match op {
            SearchOperation::Rebuild { docs } => {
                assert_eq!(docs.len(), 2);
                assert_eq!(docs[0].message_id, "m1");
                assert_eq!(docs[1].message_id, "m2");
            }
            _ => panic!("expected rebuild operation"),
        }
    }

    #[test]
    fn supports_empty_rebuild_docs() {
        let op = build_search_rebuild_operation(Vec::new());

        match op {
            SearchOperation::Rebuild { docs } => assert!(docs.is_empty()),
            _ => panic!("expected rebuild operation"),
        }
    }
}
