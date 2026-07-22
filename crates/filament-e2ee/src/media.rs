//! MLS-bound media key scheduling for native SFrame integration.
//!
//! This module deliberately stops at the vetted MLS exporter boundary. Frame
//! protection must be supplied by a reviewed SFrame implementation; Filament
//! does not define a bespoke frame-encryption construction.

use core::fmt;

use filament_core::GroupId;
use zeroize::Zeroizing;

use crate::{
    conversation::{MlsConversation, PendingGroupCommit},
    error::{ConversationError, MediaError},
    keypackage::MlsDevice,
};

/// Domain-separation label for the MLS media exporter.
pub(crate) const MEDIA_EXPORTER_LABEL: &str = "filament media sframe v1";
/// Bytes exported for the native SFrame key schedule.
pub(crate) const MEDIA_EXPORTER_SECRET_BYTES: usize = 32;
const MEDIA_EXPORTER_CONTEXT_VERSION: u16 = 1;

/// Minimum permitted interval between periodic media update commits.
pub const MIN_MEDIA_REKEY_INTERVAL_SECS: u64 = 60;
/// Maximum permitted interval between periodic media update commits.
pub const MAX_MEDIA_REKEY_INTERVAL_SECS: u64 = 3_600;
/// Default interval for periodic media update commits.
pub const DEFAULT_MEDIA_REKEY_INTERVAL_SECS: u64 = 900;

/// A bounded interval for periodic media update commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaRekeyInterval(u64);

impl MediaRekeyInterval {
    /// Return the interval in seconds.
    #[must_use]
    pub const fn as_secs(self) -> u64 {
        self.0
    }
}

impl Default for MediaRekeyInterval {
    fn default() -> Self {
        Self(DEFAULT_MEDIA_REKEY_INTERVAL_SECS)
    }
}

impl TryFrom<u64> for MediaRekeyInterval {
    type Error = MediaError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if !(MIN_MEDIA_REKEY_INTERVAL_SECS..=MAX_MEDIA_REKEY_INTERVAL_SECS).contains(&value) {
            return Err(MediaError::InvalidRekeyInterval);
        }
        Ok(Self(value))
    }
}

/// Secret exported from one authenticated MLS group epoch.
///
/// The raw bytes are intentionally unavailable through the public API. A
/// native SFrame adapter in this crate can consume them without allowing key
/// material to cross the desktop IPC boundary or enter the JavaScript heap.
pub struct MediaEpochSecret {
    group_id: GroupId,
    epoch: u64,
    secret: Zeroizing<[u8; MEDIA_EXPORTER_SECRET_BYTES]>,
}

impl MediaEpochSecret {
    pub(crate) fn new(
        group_id: GroupId,
        epoch: u64,
        secret: [u8; MEDIA_EXPORTER_SECRET_BYTES],
    ) -> Self {
        Self {
            group_id,
            epoch,
            secret: Zeroizing::new(secret),
        }
    }

    /// Authenticated MLS group that produced this secret.
    #[must_use]
    pub const fn group_id(&self) -> GroupId {
        self.group_id
    }

    /// Authenticated MLS epoch that produced this secret.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    #[cfg(feature = "livekit-media")]
    pub(crate) fn key_bytes(&self) -> &[u8; MEDIA_EXPORTER_SECRET_BYTES] {
        &self.secret
    }

    #[cfg(test)]
    pub(crate) fn secret(&self) -> &[u8; MEDIA_EXPORTER_SECRET_BYTES] {
        &self.secret
    }
}

impl fmt::Debug for MediaEpochSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MediaEpochSecret")
            .field("group_id", &self.group_id)
            .field("epoch", &self.epoch)
            .field("secret_bytes", &self.secret.len())
            .field("secret", &"<MLS exporter secret omitted>")
            .finish()
    }
}

/// Outcome of polling the periodic media rekey scheduler.
#[derive(Debug)]
pub enum MediaRekeyAction {
    /// The deadline has not elapsed and the current key remains valid.
    NotDue,
    /// Another authenticated commit already advanced the group, so media must
    /// switch to the exporter secret for this epoch.
    EpochAdvanced { epoch: u64 },
    /// A self-update commit was staged and must be ordered by the Delivery
    /// Service before its media epoch is used.
    Commit(PendingGroupCommit),
}

/// Secret-free scheduler for bounded periodic media rekeys.
///
/// Membership commits are observed as epoch changes and reset the periodic
/// deadline. A staged self-update does not advance the media epoch until the
/// normal acceptance-gated commit path merges it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeriodicMediaRekey {
    interval: MediaRekeyInterval,
    observed_epoch: u64,
    next_rekey_at_unix: u64,
}

impl PeriodicMediaRekey {
    /// Start a scheduler from the conversation's currently authenticated epoch.
    ///
    /// # Errors
    /// Rejects a different or inactive local device, and returns
    /// [`MediaError::TimestampOverflow`] if the bounded deadline cannot be
    /// represented.
    pub fn new(
        conversation: &MlsConversation,
        device: &MlsDevice,
        interval: MediaRekeyInterval,
        now_unix: u64,
    ) -> Result<Self, MediaError> {
        conversation.ensure_media_access(device)?;
        Ok(Self {
            interval,
            observed_epoch: conversation.epoch(),
            next_rekey_at_unix: deadline(now_unix, interval)?,
        })
    }

    /// Next periodic rekey deadline, expressed as Unix seconds.
    #[must_use]
    pub const fn next_rekey_at_unix(&self) -> u64 {
        self.next_rekey_at_unix
    }

    /// Poll for an authenticated epoch change or a due periodic update.
    ///
    /// # Errors
    /// Fails closed if the deadline overflows or the conversation cannot stage
    /// an update (for example, because another commit is pending).
    pub fn poll(
        &mut self,
        conversation: &mut MlsConversation,
        device: &MlsDevice,
        now_unix: u64,
    ) -> Result<MediaRekeyAction, MediaError> {
        conversation.ensure_media_access(device)?;
        let current_epoch = conversation.epoch();
        if current_epoch != self.observed_epoch {
            self.observed_epoch = current_epoch;
            self.next_rekey_at_unix = deadline(now_unix, self.interval)?;
            return Ok(MediaRekeyAction::EpochAdvanced {
                epoch: current_epoch,
            });
        }
        if now_unix < self.next_rekey_at_unix {
            return Ok(MediaRekeyAction::NotDue);
        }
        conversation
            .create_self_update(device)
            .map(MediaRekeyAction::Commit)
            .map_err(MediaError::from)
    }
}

pub(crate) fn exporter_context(group_id: GroupId, epoch: u64) -> Vec<u8> {
    let group_id = group_id.to_string();
    let mut context = Vec::with_capacity(2 + group_id.len() + 8);
    context.extend_from_slice(&MEDIA_EXPORTER_CONTEXT_VERSION.to_be_bytes());
    context.extend_from_slice(group_id.as_bytes());
    context.extend_from_slice(&epoch.to_be_bytes());
    context
}

fn deadline(now_unix: u64, interval: MediaRekeyInterval) -> Result<u64, MediaError> {
    now_unix
        .checked_add(interval.as_secs())
        .ok_or(MediaError::TimestampOverflow)
}

impl From<ConversationError> for MediaError {
    fn from(error: ConversationError) -> Self {
        Self::Conversation(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_is_bounded() {
        assert_eq!(
            MediaRekeyInterval::try_from(MIN_MEDIA_REKEY_INTERVAL_SECS - 1).unwrap_err(),
            MediaError::InvalidRekeyInterval
        );
        assert_eq!(
            MediaRekeyInterval::try_from(MAX_MEDIA_REKEY_INTERVAL_SECS + 1).unwrap_err(),
            MediaError::InvalidRekeyInterval
        );
        assert_eq!(
            MediaRekeyInterval::try_from(DEFAULT_MEDIA_REKEY_INTERVAL_SECS)
                .unwrap()
                .as_secs(),
            DEFAULT_MEDIA_REKEY_INTERVAL_SECS
        );
    }

    #[test]
    fn deadline_overflow_fails_closed() {
        let interval = MediaRekeyInterval::default();
        assert_eq!(
            deadline(u64::MAX, interval).unwrap_err(),
            MediaError::TimestampOverflow
        );
    }
}
