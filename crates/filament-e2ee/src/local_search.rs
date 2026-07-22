//! Ephemeral full-text search over authenticated E2EE history.
//!
//! The canonical records remain in the encrypted local store. This module
//! materializes current message bodies, applies authenticated edits/deletes and
//! expiry, and builds a Tantivy index in memory only. No plaintext index files
//! or server requests are created.

use std::collections::{HashMap, HashSet};

use filament_core::{GroupId, MarkdownToken, UserId};
use tantivy::{
    collector::TopDocs,
    query::{BooleanQuery, Occur, Query, TermQuery},
    schema::{Field, IndexRecordOption, Schema, Value, STORED, STRING, TEXT},
    tokenizer::{LowerCaser, RemoveLongFilter, SimpleTokenizer, TextAnalyzer, TokenStream},
    Index, IndexReader, TantivyDocument, Term,
};

use crate::{
    durable_mailbox::{decode_stored_message, parse_history_key},
    ChatMessageBody, EncryptedChatEvent, EncryptedMessageId, LocalKeyStore, LocalSearchError,
    VersionedApplicationEvent, MAX_STORE_ENTRIES,
};

/// Maximum UTF-8 bytes accepted in one literal local-search query.
pub const MAX_LOCAL_SEARCH_QUERY_BYTES: usize = 256;
/// Maximum analyzed terms accepted in one local-search query.
pub const MAX_LOCAL_SEARCH_QUERY_TERMS: usize = 16;
/// Maximum hits returned by one local-search operation.
pub const MAX_LOCAL_SEARCH_RESULTS: usize = 50;

const INDEX_WRITER_HEAP_BYTES: usize = 20_000_000;
const MAX_UNIX_TIMESTAMP: i64 = 253_402_300_799;

/// A validated, literal full-text query with an optional conversation scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSearchQuery {
    text: String,
    group_id: Option<GroupId>,
    limit: usize,
}

impl LocalSearchQuery {
    /// Validate a bounded query. Query-parser operators have no special meaning.
    ///
    /// # Errors
    /// Rejects empty, control-containing, oversized, or token-heavy input and
    /// result limits outside `1..=50`.
    pub fn new(
        text: impl Into<String>,
        group_id: Option<GroupId>,
        limit: usize,
    ) -> Result<Self, LocalSearchError> {
        let text = text.into();
        if text.len() > MAX_LOCAL_SEARCH_QUERY_BYTES || text.chars().any(char::is_control) {
            return Err(LocalSearchError::InvalidQuery);
        }
        let text = text.trim();
        if text.is_empty() {
            return Err(LocalSearchError::InvalidQuery);
        }
        if !(1..=MAX_LOCAL_SEARCH_RESULTS).contains(&limit) {
            return Err(LocalSearchError::LimitExceeded);
        }
        let terms = analyze_terms(text)?;
        if terms.is_empty() {
            return Err(LocalSearchError::InvalidQuery);
        }
        if terms.len() > MAX_LOCAL_SEARCH_QUERY_TERMS {
            return Err(LocalSearchError::LimitExceeded);
        }
        Ok(Self {
            text: text.to_owned(),
            group_id,
            limit,
        })
    }
}

/// One current, non-expired local message returned as HTML-free UI tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSearchHit {
    pub group_id: GroupId,
    pub message_id: EncryptedMessageId,
    pub sender_user_id: UserId,
    pub created_at_unix: i64,
    pub body: Vec<MarkdownToken>,
}

#[derive(Clone)]
struct MaterializedMessage {
    group_id: GroupId,
    message_id: EncryptedMessageId,
    sender_user_id: UserId,
    created_at_unix: i64,
    body: ChatMessageBody,
}

#[derive(Clone, Copy)]
struct SearchFields {
    ordinal: Field,
    group_id: Field,
    body: Field,
}

/// A bounded, process-local search index rebuilt from encrypted history.
pub struct LocalSearchIndex {
    reader: IndexReader,
    fields: SearchFields,
    messages: Vec<MaterializedMessage>,
}

impl core::fmt::Debug for LocalSearchIndex {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LocalSearchIndex")
            .field("message_count", &self.messages.len())
            .field("contents", &"<authenticated plaintext omitted>")
            .finish_non_exhaustive()
    }
}

impl LocalSearchIndex {
    /// Rebuild an in-memory index from authenticated, encrypted local history.
    ///
    /// Expired records are excluded before any plaintext enters the index.
    /// The caller should separately run the bounded hard-deletion sweep; this
    /// read-only rebuild never makes index durability depend on a store write.
    ///
    /// # Errors
    /// Fails closed on an invalid clock, corrupt history, store failure, or
    /// Tantivy failure. Non-chat application payloads are not searchable.
    pub fn rebuild(store: &dyn LocalKeyStore, now_unix: i64) -> Result<Self, LocalSearchError> {
        if !(0..=MAX_UNIX_TIMESTAMP).contains(&now_unix) {
            return Err(LocalSearchError::InvalidHistory);
        }
        let messages = materialize_messages(store, now_unix)?;
        let (schema, fields) = search_schema();
        let index = Index::create_in_ram(schema);
        let mut writer = index
            .writer(INDEX_WRITER_HEAP_BYTES)
            .map_err(|_| LocalSearchError::IndexUnavailable)?;
        for (ordinal, message) in messages.iter().enumerate() {
            let ordinal = u64::try_from(ordinal).map_err(|_| LocalSearchError::LimitExceeded)?;
            let mut document = TantivyDocument::default();
            document.add_u64(fields.ordinal, ordinal);
            document.add_text(fields.group_id, message.group_id.to_string());
            document.add_text(fields.body, message.body.as_str());
            writer
                .add_document(document)
                .map_err(|_| LocalSearchError::IndexUnavailable)?;
        }
        writer
            .commit()
            .map_err(|_| LocalSearchError::IndexUnavailable)?;
        let reader = index
            .reader()
            .map_err(|_| LocalSearchError::IndexUnavailable)?;
        reader
            .reload()
            .map_err(|_| LocalSearchError::IndexUnavailable)?;
        Ok(Self {
            reader,
            fields,
            messages,
        })
    }

    /// Search current local E2EE message bodies without contacting the server.
    ///
    /// Every analyzed query term is required. Operators, field selectors, and
    /// wildcards are treated as ordinary punctuation rather than query syntax.
    ///
    /// # Errors
    /// Returns an opaque index error if the in-memory reader is unavailable or
    /// a stored document violates the rebuild invariant.
    pub fn search(
        &self,
        query: &LocalSearchQuery,
    ) -> Result<Vec<LocalSearchHit>, LocalSearchError> {
        let terms = analyze_terms(&query.text)?;
        if terms.is_empty() || terms.len() > MAX_LOCAL_SEARCH_QUERY_TERMS {
            return Err(LocalSearchError::InvalidQuery);
        }
        let mut clauses = terms
            .into_iter()
            .map(|term| {
                (
                    Occur::Must,
                    Box::new(TermQuery::new(
                        Term::from_field_text(self.fields.body, &term),
                        IndexRecordOption::WithFreqs,
                    )) as Box<dyn Query>,
                )
            })
            .collect::<Vec<_>>();
        if let Some(group_id) = query.group_id {
            clauses.push((
                Occur::Must,
                Box::new(TermQuery::new(
                    Term::from_field_text(self.fields.group_id, &group_id.to_string()),
                    IndexRecordOption::Basic,
                )),
            ));
        }
        let searcher = self.reader.searcher();
        let documents = searcher
            .search(
                &BooleanQuery::from(clauses),
                &TopDocs::with_limit(query.limit),
            )
            .map_err(|_| LocalSearchError::IndexUnavailable)?;
        let mut hits = Vec::with_capacity(documents.len());
        for (_, address) in documents {
            let document = searcher
                .doc::<TantivyDocument>(address)
                .map_err(|_| LocalSearchError::IndexUnavailable)?;
            let ordinal = document
                .get_first(self.fields.ordinal)
                .and_then(|value| value.as_u64())
                .and_then(|value| usize::try_from(value).ok())
                .ok_or(LocalSearchError::InvalidHistory)?;
            let message = self
                .messages
                .get(ordinal)
                .ok_or(LocalSearchError::InvalidHistory)?;
            hits.push(LocalSearchHit {
                group_id: message.group_id,
                message_id: message.message_id,
                sender_user_id: message.sender_user_id,
                created_at_unix: message.created_at_unix,
                body: message.body.safe_markdown_tokens(),
            });
        }
        Ok(hits)
    }

    /// Number of current message bodies held by the ephemeral index.
    #[must_use]
    pub const fn message_count(&self) -> usize {
        self.messages.len()
    }
}

fn materialize_messages(
    store: &dyn LocalKeyStore,
    now_unix: i64,
) -> Result<Vec<MaterializedMessage>, LocalSearchError> {
    let keys = store.list_keys()?;
    if keys.len() > MAX_STORE_ENTRIES {
        return Err(LocalSearchError::LimitExceeded);
    }
    let mut history = Vec::new();
    for key in keys {
        let Some((group_id, transport_id)) = parse_history_key(&key)? else {
            continue;
        };
        let encoded = store.load(&key)?;
        let stored = decode_stored_message(group_id, &transport_id, &encoded)
            .map_err(|_| LocalSearchError::InvalidHistory)?;
        if stored
            .expires_at_unix
            .is_some_and(|expires_at| expires_at <= now_unix)
        {
            continue;
        }
        history.push((transport_id, stored));
    }
    history.sort_by(|left, right| {
        left.1
            .created_at_unix
            .cmp(&right.1.created_at_unix)
            .then_with(|| left.0.cmp(&right.0))
    });

    let mut seen_events = HashSet::new();
    let mut current = HashMap::<(GroupId, EncryptedMessageId), MaterializedMessage>::new();
    for (_, stored) in history {
        let Ok(application) = VersionedApplicationEvent::decode(&stored.message.plaintext) else {
            continue;
        };
        if !seen_events.insert((stored.group_id, application.event_id)) {
            continue;
        }
        let sender = stored.message.sender_user_id;
        match application.event {
            EncryptedChatEvent::Message {
                message_id, body, ..
            }
            | EncryptedChatEvent::Attachments {
                message_id,
                body: Some(body),
                ..
            } => {
                current
                    .entry((stored.group_id, message_id))
                    .or_insert(MaterializedMessage {
                        group_id: stored.group_id,
                        message_id,
                        sender_user_id: sender,
                        created_at_unix: stored.created_at_unix,
                        body,
                    });
            }
            EncryptedChatEvent::Edit {
                target_message_id,
                body,
            } => {
                if let Some(message) = current.get_mut(&(stored.group_id, target_message_id)) {
                    if message.sender_user_id == sender {
                        message.body = body;
                    }
                }
            }
            EncryptedChatEvent::Delete { target_message_id } => {
                let key = (stored.group_id, target_message_id);
                if current
                    .get(&key)
                    .is_some_and(|message| message.sender_user_id == sender)
                {
                    current.remove(&key);
                }
            }
            EncryptedChatEvent::SetDisappearingTimer { .. }
            | EncryptedChatEvent::Attachments { body: None, .. }
            | EncryptedChatEvent::Reaction { .. }
            | EncryptedChatEvent::Pin { .. } => {}
        }
    }
    let mut messages = current.into_values().collect::<Vec<_>>();
    messages.sort_by(|left, right| {
        right
            .created_at_unix
            .cmp(&left.created_at_unix)
            .then_with(|| {
                left.message_id
                    .to_string()
                    .cmp(&right.message_id.to_string())
            })
    });
    Ok(messages)
}

fn analyze_terms(text: &str) -> Result<Vec<String>, LocalSearchError> {
    let mut analyzer = TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(RemoveLongFilter::limit(40))
        .filter(LowerCaser)
        .build();
    let mut stream = analyzer.token_stream(text);
    let mut terms = Vec::new();
    let mut term_count = 0_usize;
    while stream.advance() {
        term_count += 1;
        if term_count > MAX_LOCAL_SEARCH_QUERY_TERMS {
            return Err(LocalSearchError::LimitExceeded);
        }
        let term = stream.token().text.clone();
        if !terms.contains(&term) {
            terms.push(term);
        }
    }
    Ok(terms)
}

fn search_schema() -> (Schema, SearchFields) {
    let mut builder = Schema::builder();
    let ordinal = builder.add_u64_field("ordinal", STORED);
    let group_id = builder.add_text_field("group_id", STRING);
    let body = builder.add_text_field("body", TEXT);
    (
        builder.build(),
        SearchFields {
            ordinal,
            group_id,
            body,
        },
    )
}

#[cfg(test)]
mod tests {
    use filament_core::{tokenize_markdown, DeviceId};
    use ulid::Ulid;

    use super::*;
    use crate::{
        durable_mailbox::history_storage_entry, ApplicationEventId, InMemoryKeyStore,
        StoredMailboxMessage,
    };

    fn body(value: &str) -> ChatMessageBody {
        ChatMessageBody::try_from(value.to_owned()).unwrap()
    }

    fn store_event(
        store: &InMemoryKeyStore,
        group_id: GroupId,
        sender_user_id: UserId,
        sender_device_id: DeviceId,
        created_at_unix: i64,
        expires_at_unix: Option<i64>,
        event: EncryptedChatEvent,
    ) {
        let application = VersionedApplicationEvent {
            event_id: ApplicationEventId::new(),
            retention_secs: None,
            event,
        };
        let record = StoredMailboxMessage {
            message_id: Ulid::new().to_string(),
            group_id,
            created_at_unix,
            expires_at_unix,
            message: crate::DecryptedApplicationMessage {
                sender_user_id,
                sender_device_id,
                generation: u64::try_from(created_at_unix).unwrap(),
                plaintext: application.encode().unwrap(),
            },
        };
        let (key, encoded) = history_storage_entry(&record).unwrap();
        store.store(key, encoded).unwrap();
    }

    #[test]
    #[allow(clippy::too_many_lines)] // The linear fixture documents event materialization order.
    fn rebuild_searches_current_scoped_history_without_server_state() {
        let store = InMemoryKeyStore::new();
        let first_group = GroupId::new();
        let second_group = GroupId::new();
        let sender = UserId::new();
        let device = DeviceId::new();
        let edited_id = EncryptedMessageId::new();
        let deleted_id = EncryptedMessageId::new();

        store_event(
            &store,
            first_group,
            sender,
            device,
            1,
            None,
            EncryptedChatEvent::Message {
                message_id: edited_id,
                body: body("old private wording"),
                reply: None,
            },
        );
        store_event(
            &store,
            first_group,
            sender,
            device,
            2,
            None,
            EncryptedChatEvent::Edit {
                target_message_id: edited_id,
                body: body("new searchable phrase"),
            },
        );
        store_event(
            &store,
            first_group,
            sender,
            device,
            3,
            None,
            EncryptedChatEvent::Attachments {
                message_id: EncryptedMessageId::new(),
                body: Some(body("searchable attachment note")),
                attachments: attachment_set(),
            },
        );
        store_event(
            &store,
            first_group,
            sender,
            device,
            4,
            None,
            EncryptedChatEvent::Message {
                message_id: deleted_id,
                body: body("searchable deleted content"),
                reply: None,
            },
        );
        store_event(
            &store,
            first_group,
            sender,
            device,
            5,
            None,
            EncryptedChatEvent::Delete {
                target_message_id: deleted_id,
            },
        );
        store_event(
            &store,
            second_group,
            sender,
            device,
            6,
            None,
            EncryptedChatEvent::Message {
                message_id: EncryptedMessageId::new(),
                body: body("searchable elsewhere"),
                reply: None,
            },
        );
        store_event(
            &store,
            first_group,
            sender,
            device,
            7,
            Some(9),
            EncryptedChatEvent::Message {
                message_id: EncryptedMessageId::new(),
                body: body("expired searchable text"),
                reply: None,
            },
        );

        let index = LocalSearchIndex::rebuild(&store, 10).unwrap();
        assert_eq!(index.message_count(), 3);
        let all = index
            .search(&LocalSearchQuery::new("SEARCHABLE", None, 10).unwrap())
            .unwrap();
        assert_eq!(all.len(), 3);
        let scoped = index
            .search(&LocalSearchQuery::new("searchable", Some(first_group), 10).unwrap())
            .unwrap();
        assert_eq!(scoped.len(), 2);
        let edited = index
            .search(&LocalSearchQuery::new("new phrase", Some(first_group), 10).unwrap())
            .unwrap();
        assert_eq!(edited.len(), 1);
        assert_eq!(edited[0].message_id, edited_id);
        assert_eq!(edited[0].body, tokenize_markdown("new searchable phrase"));
        assert!(index
            .search(&LocalSearchQuery::new("old", None, 10).unwrap())
            .unwrap()
            .is_empty());
        assert!(index
            .search(&LocalSearchQuery::new("deleted", None, 10).unwrap())
            .unwrap()
            .is_empty());
        assert!(index
            .search(&LocalSearchQuery::new("expired", None, 10).unwrap())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn edits_and_deletes_cannot_cross_authenticated_authors() {
        let store = InMemoryKeyStore::new();
        let group_id = GroupId::new();
        let author = UserId::new();
        let attacker = UserId::new();
        let device = DeviceId::new();
        let message_id = EncryptedMessageId::new();
        store_event(
            &store,
            group_id,
            author,
            device,
            1,
            None,
            EncryptedChatEvent::Message {
                message_id,
                body: body("owner searchable content"),
                reply: None,
            },
        );
        store_event(
            &store,
            group_id,
            attacker,
            device,
            2,
            None,
            EncryptedChatEvent::Edit {
                target_message_id: message_id,
                body: body("attacker replacement"),
            },
        );
        store_event(
            &store,
            group_id,
            attacker,
            device,
            3,
            None,
            EncryptedChatEvent::Delete {
                target_message_id: message_id,
            },
        );

        let index = LocalSearchIndex::rebuild(&store, 10).unwrap();
        let hits = index
            .search(&LocalSearchQuery::new("owner searchable", None, 10).unwrap())
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].sender_user_id, author);
        assert!(index
            .search(&LocalSearchQuery::new("attacker", None, 10).unwrap())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn query_and_debug_surfaces_are_bounded_and_redacted() {
        assert_eq!(
            LocalSearchQuery::new("", None, 1).unwrap_err(),
            LocalSearchError::InvalidQuery
        );
        assert_eq!(
            LocalSearchQuery::new("\nsecret", None, 1).unwrap_err(),
            LocalSearchError::InvalidQuery
        );
        assert_eq!(
            LocalSearchQuery::new("a".repeat(MAX_LOCAL_SEARCH_QUERY_BYTES + 1), None, 1)
                .unwrap_err(),
            LocalSearchError::InvalidQuery
        );
        assert_eq!(
            LocalSearchQuery::new("word", None, 0).unwrap_err(),
            LocalSearchError::LimitExceeded
        );
        assert_eq!(
            LocalSearchQuery::new(
                (0..=MAX_LOCAL_SEARCH_QUERY_TERMS)
                    .map(|index| format!("term{index}"))
                    .collect::<Vec<_>>()
                    .join(" "),
                None,
                1,
            )
            .unwrap_err(),
            LocalSearchError::LimitExceeded
        );

        let store = InMemoryKeyStore::new();
        let group_id = GroupId::new();
        store_event(
            &store,
            group_id,
            UserId::new(),
            DeviceId::new(),
            1,
            None,
            EncryptedChatEvent::Message {
                message_id: EncryptedMessageId::new(),
                body: body("<script>private needle</script>"),
                reply: None,
            },
        );
        let index = LocalSearchIndex::rebuild(&store, 2).unwrap();
        let hit = index
            .search(&LocalSearchQuery::new("needle", None, 1).unwrap())
            .unwrap()
            .remove(0);
        let rendered = serde_json::to_string(&hit.body).unwrap();
        assert!(!rendered.contains("<script>"));
        let debug = format!("{index:?}");
        assert!(!debug.contains("private"));
        assert!(!debug.contains("needle"));
    }

    fn attachment_set() -> crate::AttachmentSet {
        let (descriptor, _) = crate::encrypt_attachment("local.txt", b"opaque").unwrap();
        crate::AttachmentSet::try_from(vec![crate::EncryptedAttachmentReference {
            file: descriptor,
            thumbnail: None,
        }])
        .unwrap()
    }
}
