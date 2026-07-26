//! Native-only WebSocket wake transport for durable E2EE mailboxes.
//!
//! Gateway payloads are never message content or authorization facts. Strictly
//! validated MLS routing events only coalesce a group wake; the authenticated
//! mailbox remains the source of opaque ciphertext and durable ordering.

use std::{
    collections::{HashSet, VecDeque},
    io::ErrorKind,
    net::{TcpStream, ToSocketAddrs as _},
    sync::{Condvar, Mutex},
    time::Duration,
};

use filament_core::{CiphersuiteId, ConversationId, DeviceId, GroupId, ProposalId, UserId};
use filament_e2ee::EncryptedMessageId;
use filament_protocol::{
    parse_envelope, MlsCommitEvent, MlsMessageEvent, MlsProposalEvent, MlsWelcomeEvent,
    MAX_EVENT_BYTES,
};
use serde::Deserialize;
use tungstenite::{
    client::IntoClientRequest as _,
    client_tls_with_config,
    http::{
        header::{HeaderValue, AUTHORIZATION},
        StatusCode,
    },
    protocol::{Message, WebSocketConfig},
    stream::MaybeTlsStream,
    WebSocket,
};
use url::Url;
use zeroize::Zeroizing;

use crate::{
    native_api::{NativeApiError, NativeApiOrigin},
    SessionToken,
};

pub(crate) const MAX_GATEWAY_WAKE_GROUPS: usize = 128;
const GATEWAY_SOCKET_TIMEOUT: Duration = Duration::from_secs(1);
const GATEWAY_CONNECT_TIMEOUT: Duration = Duration::from_secs(7);
const MAX_GATEWAY_RESOLVED_ADDRESSES: usize = 4;
const GATEWAY_WRITE_BUFFER_BYTES: usize = MAX_EVENT_BYTES + 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeGatewayError {
    Unavailable,
    Rejected,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum NativeGatewayFrame {
    Text(Vec<u8>),
    Activity,
    Timeout,
    Closed,
}

pub(crate) trait NativeGatewayConnection: Send {
    fn read_frame(&mut self) -> Result<NativeGatewayFrame, NativeGatewayError>;
}

pub(crate) trait NativeGatewayConnector: Send + Sync + 'static {
    fn connect(
        &self,
        access_token: &SessionToken,
    ) -> Result<Box<dyn NativeGatewayConnection>, NativeGatewayError>;
}

pub(crate) struct TungsteniteNativeGatewayConnector {
    endpoint: Url,
}

impl core::fmt::Debug for TungsteniteNativeGatewayConnector {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("TungsteniteNativeGatewayConnector(<origin redacted>)")
    }
}

impl TungsteniteNativeGatewayConnector {
    pub(crate) fn from_build_config() -> Result<Self, NativeGatewayError> {
        let endpoint = NativeApiOrigin::from_build_config()
            .and_then(|origin| origin.gateway_endpoint())
            .map_err(map_native_api_error)?;
        Ok(Self { endpoint })
    }
}

impl NativeGatewayConnector for TungsteniteNativeGatewayConnector {
    fn connect(
        &self,
        access_token: &SessionToken,
    ) -> Result<Box<dyn NativeGatewayConnection>, NativeGatewayError> {
        let mut request = self
            .endpoint
            .as_str()
            .into_client_request()
            .map_err(|_| NativeGatewayError::Rejected)?;
        let bearer = Zeroizing::new(format!("Bearer {}", access_token.expose()));
        let mut authorization =
            HeaderValue::from_str(&bearer).map_err(|_| NativeGatewayError::Rejected)?;
        authorization.set_sensitive(true);
        request.headers_mut().insert(AUTHORIZATION, authorization);

        let config = WebSocketConfig::default()
            .read_buffer_size(8 * 1024)
            .write_buffer_size(0)
            .max_write_buffer_size(GATEWAY_WRITE_BUFFER_BYTES)
            .max_message_size(Some(MAX_EVENT_BYTES))
            .max_frame_size(Some(MAX_EVENT_BYTES));
        let host = self
            .endpoint
            .host_str()
            .ok_or(NativeGatewayError::Rejected)?;
        let port = self
            .endpoint
            .port_or_known_default()
            .ok_or(NativeGatewayError::Rejected)?;
        let per_address_timeout = GATEWAY_CONNECT_TIMEOUT / 4;
        let addresses = (host, port)
            .to_socket_addrs()
            .map_err(|_| NativeGatewayError::Unavailable)?;
        let mut stream = None;
        for address in addresses.take(MAX_GATEWAY_RESOLVED_ADDRESSES) {
            if let Ok(connected) = TcpStream::connect_timeout(&address, per_address_timeout) {
                stream = Some(connected);
                break;
            }
        }
        let stream = stream.ok_or(NativeGatewayError::Unavailable)?;
        stream
            .set_read_timeout(Some(GATEWAY_CONNECT_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(GATEWAY_CONNECT_TIMEOUT)))
            .map_err(|_| NativeGatewayError::Unavailable)?;
        let (mut socket, response) = client_tls_with_config(request, stream, Some(config), None)
            .map_err(|_| NativeGatewayError::Unavailable)?;
        if response.status() != StatusCode::SWITCHING_PROTOCOLS {
            return Err(NativeGatewayError::Rejected);
        }
        configure_socket_timeouts(socket.get_mut())?;
        Ok(Box::new(TungsteniteGatewayConnection { socket }))
    }
}

struct TungsteniteGatewayConnection {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
}

impl NativeGatewayConnection for TungsteniteGatewayConnection {
    fn read_frame(&mut self) -> Result<NativeGatewayFrame, NativeGatewayError> {
        match self.socket.read() {
            Ok(Message::Text(payload)) => {
                if payload.len() > MAX_EVENT_BYTES {
                    return Err(NativeGatewayError::Rejected);
                }
                Ok(NativeGatewayFrame::Text(payload.as_bytes().to_vec()))
            }
            Ok(Message::Ping(_) | Message::Pong(_)) => Ok(NativeGatewayFrame::Activity),
            Ok(Message::Close(_)) => Ok(NativeGatewayFrame::Closed),
            Ok(Message::Binary(_) | Message::Frame(_))
            | Err(
                tungstenite::Error::Capacity(_)
                | tungstenite::Error::Protocol(_)
                | tungstenite::Error::Utf8(_),
            ) => Err(NativeGatewayError::Rejected),
            Err(tungstenite::Error::Io(error))
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                Ok(NativeGatewayFrame::Timeout)
            }
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                Ok(NativeGatewayFrame::Closed)
            }
            Err(_) => Err(NativeGatewayError::Unavailable),
        }
    }
}

fn configure_socket_timeouts(
    stream: &mut MaybeTlsStream<TcpStream>,
) -> Result<(), NativeGatewayError> {
    let socket = match stream {
        MaybeTlsStream::Rustls(stream) => &stream.sock,
        _ => return Err(NativeGatewayError::Rejected),
    };
    socket
        .set_read_timeout(Some(GATEWAY_SOCKET_TIMEOUT))
        .and_then(|()| socket.set_write_timeout(Some(GATEWAY_SOCKET_TIMEOUT)))
        .map_err(|_| NativeGatewayError::Unavailable)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GatewayWake {
    Ready(UserId),
    Group(GroupId),
    Ignore,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadyEvent {
    user_id: String,
}

pub(crate) fn decode_gateway_wake(payload: &[u8]) -> Result<GatewayWake, NativeGatewayError> {
    let envelope = parse_envelope(payload).map_err(|_| NativeGatewayError::Rejected)?;
    match envelope.t.as_str() {
        "ready" => {
            let event: ReadyEvent =
                serde_json::from_value(envelope.d).map_err(|_| NativeGatewayError::Rejected)?;
            let user_id =
                UserId::try_from(event.user_id).map_err(|_| NativeGatewayError::Rejected)?;
            Ok(GatewayWake::Ready(user_id))
        }
        "mls_message" => {
            let event: MlsMessageEvent =
                serde_json::from_value(envelope.d).map_err(|_| NativeGatewayError::Rejected)?;
            validate_message_wake(event).map(GatewayWake::Group)
        }
        "mls_commit" => {
            let event: MlsCommitEvent =
                serde_json::from_value(envelope.d).map_err(|_| NativeGatewayError::Rejected)?;
            validate_commit_wake(event).map(GatewayWake::Group)
        }
        "mls_welcome" => {
            let event: MlsWelcomeEvent =
                serde_json::from_value(envelope.d).map_err(|_| NativeGatewayError::Rejected)?;
            validate_welcome_wake(&event).map(GatewayWake::Group)
        }
        "mls_proposal" => {
            let event: MlsProposalEvent =
                serde_json::from_value(envelope.d).map_err(|_| NativeGatewayError::Rejected)?;
            validate_proposal_wake(event).map(GatewayWake::Group)
        }
        _ => Ok(GatewayWake::Ignore),
    }
}

fn validate_message_wake(event: MlsMessageEvent) -> Result<GroupId, NativeGatewayError> {
    let group_id = parse_common_wake(
        &event.group_id,
        &event.conversation_id,
        event.created_at_unix,
    )?;
    EncryptedMessageId::try_from(event.message_id).map_err(|_| NativeGatewayError::Rejected)?;
    DeviceId::try_from(event.sender_device_id).map_err(|_| NativeGatewayError::Rejected)?;
    CiphersuiteId::try_from(event.suite_id).map_err(|_| NativeGatewayError::Rejected)?;
    Ok(group_id)
}

fn validate_commit_wake(event: MlsCommitEvent) -> Result<GroupId, NativeGatewayError> {
    let group_id = parse_common_wake(
        &event.group_id,
        &event.conversation_id,
        event.created_at_unix,
    )?;
    DeviceId::try_from(event.committer_device_id).map_err(|_| NativeGatewayError::Rejected)?;
    if event.prior_epoch.checked_add(1) != Some(event.epoch) {
        return Err(NativeGatewayError::Rejected);
    }
    Ok(group_id)
}

fn validate_welcome_wake(event: &MlsWelcomeEvent) -> Result<GroupId, NativeGatewayError> {
    let group_id = parse_common_wake(
        &event.group_id,
        &event.conversation_id,
        event.created_at_unix,
    )?;
    CiphersuiteId::try_from(event.suite_id).map_err(|_| NativeGatewayError::Rejected)?;
    Ok(group_id)
}

fn validate_proposal_wake(event: MlsProposalEvent) -> Result<GroupId, NativeGatewayError> {
    let group_id = parse_common_wake(
        &event.group_id,
        &event.conversation_id,
        event.created_at_unix,
    )?;
    ProposalId::try_from(event.proposal_id).map_err(|_| NativeGatewayError::Rejected)?;
    let has_proposer = event.proposer_device_id.is_some();
    if let Some(device_id) = event.proposer_device_id {
        DeviceId::try_from(device_id).map_err(|_| NativeGatewayError::Rejected)?;
    }
    if has_proposer == event.external_sender_index.is_some()
        || event
            .reconciliation_deadline_unix
            .is_some_and(|deadline| !valid_unix_timestamp(deadline))
    {
        return Err(NativeGatewayError::Rejected);
    }
    Ok(group_id)
}

fn parse_common_wake(
    group_id: &str,
    conversation_id: &str,
    created_at_unix: i64,
) -> Result<GroupId, NativeGatewayError> {
    if !valid_unix_timestamp(created_at_unix) {
        return Err(NativeGatewayError::Rejected);
    }
    ConversationId::try_from(conversation_id.to_owned())
        .map_err(|_| NativeGatewayError::Rejected)?;
    GroupId::try_from(group_id.to_owned()).map_err(|_| NativeGatewayError::Rejected)
}

const fn valid_unix_timestamp(timestamp: i64) -> bool {
    timestamp >= 0 && timestamp <= 253_402_300_799
}

struct GatewayWakeState {
    groups: VecDeque<GroupId>,
    unique: HashSet<GroupId>,
}

pub(crate) struct GatewayWakeQueue {
    state: Mutex<GatewayWakeState>,
    changed: Condvar,
}

impl GatewayWakeQueue {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(GatewayWakeState {
                groups: VecDeque::new(),
                unique: HashSet::new(),
            }),
            changed: Condvar::new(),
        }
    }

    pub(crate) fn enqueue(&self, group_id: GroupId) -> Result<bool, NativeGatewayError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| NativeGatewayError::Unavailable)?;
        if state.unique.contains(&group_id) {
            return Ok(false);
        }
        if state.groups.len() >= MAX_GATEWAY_WAKE_GROUPS {
            return Err(NativeGatewayError::Rejected);
        }
        state.groups.push_back(group_id);
        state.unique.insert(group_id);
        self.changed.notify_one();
        Ok(true)
    }

    pub(crate) fn take(&self, timeout: Duration) -> Result<Option<GroupId>, NativeGatewayError> {
        let state = self
            .state
            .lock()
            .map_err(|_| NativeGatewayError::Unavailable)?;
        let (mut state, _) = self
            .changed
            .wait_timeout_while(state, timeout, |state| state.groups.is_empty())
            .map_err(|_| NativeGatewayError::Unavailable)?;
        let Some(group_id) = state.groups.pop_front() else {
            return Ok(None);
        };
        if !state.unique.remove(&group_id) {
            return Err(NativeGatewayError::Rejected);
        }
        Ok(Some(group_id))
    }
}

const fn map_native_api_error(error: NativeApiError) -> NativeGatewayError {
    match error {
        NativeApiError::Unavailable => NativeGatewayError::Unavailable,
        NativeApiError::Rejected | NativeApiError::EpochConflict => NativeGatewayError::Rejected,
    }
}

#[cfg(test)]
mod tests {
    use filament_core::{ConversationId, DeviceId, GroupId, UserId};
    use filament_e2ee::{EncryptedMessageId, MlsDevice, RootIdentityKey};
    use filament_protocol::{Envelope, EventType, MlsMessageEvent, PROTOCOL_VERSION};

    use super::*;

    fn message_payload(group_id: GroupId) -> Vec<u8> {
        let root = RootIdentityKey::generate();
        let sender = MlsDevice::generate(UserId::new(), DeviceId::new(), &root).unwrap();
        serde_json::to_vec(&Envelope {
            v: PROTOCOL_VERSION,
            t: EventType::try_from(String::from("mls_message")).unwrap(),
            d: MlsMessageEvent {
                group_id: group_id.to_string(),
                conversation_id: ConversationId::new().to_string(),
                message_id: EncryptedMessageId::new().to_string(),
                epoch: 1,
                suite_id: CiphersuiteId::MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_ED25519
                    .as_u16(),
                sender_device_id: sender.device_id().to_string(),
                created_at_unix: 100,
            },
        })
        .unwrap()
    }

    #[test]
    fn gateway_wakes_are_strict_bounded_and_ignore_forward_compatible_events() {
        let group_id = GroupId::new();
        assert_eq!(
            decode_gateway_wake(&message_payload(group_id)),
            Ok(GatewayWake::Group(group_id))
        );
        assert_eq!(
            decode_gateway_wake(br#"{"v":1,"t":"future_event","d":{"bounded":true}}"#),
            Ok(GatewayWake::Ignore)
        );
        assert_eq!(
            decode_gateway_wake(br#"{"v":1,"t":"mls_message","d":{"group_id":"bad"}}"#),
            Err(NativeGatewayError::Rejected)
        );
        assert_eq!(
            decode_gateway_wake(&vec![b'x'; MAX_EVENT_BYTES + 1]),
            Err(NativeGatewayError::Rejected)
        );
    }

    #[test]
    fn wake_queue_coalesces_groups_and_rejects_distinct_overflow() {
        let queue = GatewayWakeQueue::new();
        let first = GroupId::new();
        assert_eq!(queue.enqueue(first), Ok(true));
        assert_eq!(queue.enqueue(first), Ok(false));
        for _ in 1..MAX_GATEWAY_WAKE_GROUPS {
            assert_eq!(queue.enqueue(GroupId::new()), Ok(true));
        }
        assert_eq!(
            queue.enqueue(GroupId::new()),
            Err(NativeGatewayError::Rejected)
        );
        assert_eq!(queue.take(Duration::ZERO), Ok(Some(first)));
    }

    #[test]
    fn gateway_connector_debug_never_exposes_the_origin() {
        let connector = TungsteniteNativeGatewayConnector {
            endpoint: Url::parse("wss://secret.example/gateway/ws").unwrap(),
        };
        assert_eq!(
            format!("{connector:?}"),
            "TungsteniteNativeGatewayConnector(<origin redacted>)"
        );
    }
}
