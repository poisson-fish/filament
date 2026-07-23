//! Delivery Service external-sender identity and constrained proposal signing.
//!
//! The Delivery Service is intentionally incapable of constructing arbitrary
//! MLS proposals through this API. It can sign only a `Remove` for an exact
//! group, epoch, and bounded leaf index. Group members still authenticate the
//! proposal and commit it; the server never advances group state itself.

use ed25519_dalek::SigningKey;
use filament_core::GroupId as FilamentGroupId;
use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_traits::types::SignatureScheme;
use zeroize::Zeroize;

use crate::{
    conversation::{DeliveryServiceIdentity, ExternalGroupProposal, MAX_MLS_GROUP_LEAVES},
    error::ConversationError,
};

/// Raw Ed25519 seed size for the Delivery Service signing identity.
pub const DELIVERY_SERVICE_SEED_BYTES: usize = 32;

/// Long-lived Delivery Service signer for MLS external Remove proposals.
///
/// Construct this once from stable, operator-managed key material and retain
/// it for the process lifetime. Rotating the key requires replacing the
/// authenticated external-sender extension in every existing MLS group.
pub struct DeliveryServiceSigner {
    signer: SignatureKeyPair,
    identity: DeliveryServiceIdentity,
}

impl DeliveryServiceSigner {
    /// Construct the signer from a raw Ed25519 seed, consuming and zeroizing
    /// the caller's transferred seed copy.
    ///
    /// # Errors
    /// Returns an opaque crypto error if the derived public identity is not a
    /// valid Delivery Service identity.
    pub fn from_seed(
        mut seed: [u8; DELIVERY_SERVICE_SEED_BYTES],
    ) -> Result<Self, ConversationError> {
        let signing_key = SigningKey::from_bytes(&seed);
        let public = signing_key.verifying_key().to_bytes();
        let signer =
            SignatureKeyPair::from_raw(SignatureScheme::ED25519, seed.to_vec(), public.to_vec());
        seed.zeroize();
        let identity = DeliveryServiceIdentity::try_new(public)?;
        Ok(Self { signer, identity })
    }

    /// Return the stable public identity clients pin into the MLS Group Context.
    #[must_use]
    pub const fn identity(&self) -> DeliveryServiceIdentity {
        self.identity
    }

    /// Create an MLS external-sender Remove proposal at extension index zero.
    ///
    /// This is the complete signing surface: no Add, Update, or arbitrary
    /// proposal signing primitive is exposed.
    ///
    /// # Errors
    /// Rejects epoch zero, leaf indices outside Filament's group cap, and any
    /// OpenMLS signing or serialization failure.
    pub fn sign_remove(
        &self,
        group_id: FilamentGroupId,
        epoch: u64,
        removed_leaf_index: u32,
    ) -> Result<ExternalGroupProposal, ConversationError> {
        if epoch == 0 || removed_leaf_index as usize >= MAX_MLS_GROUP_LEAVES {
            return Err(ConversationError::LimitExceeded);
        }
        let openmls_group_id =
            openmls::prelude::GroupId::from_slice(group_id.to_string().as_bytes());
        let proposal = ExternalProposal::new_remove::<openmls_rust_crypto::OpenMlsRustCrypto>(
            LeafNodeIndex::new(removed_leaf_index),
            openmls_group_id,
            GroupEpoch::from(epoch),
            &self.signer,
            SenderExtensionIndex::new(0),
        )
        .map_err(|_| ConversationError::CryptoError)?;
        let proposal_blob = proposal
            .to_bytes()
            .map_err(|_| ConversationError::SerializationFailed)?;
        if proposal_blob.is_empty() || proposal_blob.len() > 65_536 {
            return Err(ConversationError::LimitExceeded);
        }
        Ok(ExternalGroupProposal {
            group_id,
            epoch,
            proposal_blob,
        })
    }
}

impl core::fmt::Debug for DeliveryServiceSigner {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("DeliveryServiceSigner(<key material redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        generate_key_package_batch, MlsConversation, MlsDevice, PinnedUserIdentity, RootIdentityKey,
    };
    use filament_core::{DeviceId, UserId};

    #[test]
    fn signer_is_stable_and_only_emits_accepted_removes() {
        let signer = DeliveryServiceSigner::from_seed([0x42; DELIVERY_SERVICE_SEED_BYTES]).unwrap();
        let restored =
            DeliveryServiceSigner::from_seed([0x42; DELIVERY_SERVICE_SEED_BYTES]).unwrap();
        assert_eq!(signer.identity(), restored.identity());
        assert_eq!(
            format!("{signer:?}"),
            "DeliveryServiceSigner(<key material redacted>)"
        );

        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let charlie_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let charlie = MlsDevice::generate(UserId::new(), DeviceId::new(), &charlie_root).unwrap();
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let charlie_pin = PinnedUserIdentity::new(charlie.user_id(), *charlie.root_key_public());
        let bob_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let charlie_package = generate_key_package_batch(&charlie, 1)
            .unwrap()
            .remove(0)
            .blob;
        let group_id = FilamentGroupId::new();
        let (mut conversation, pending) = MlsConversation::create_group_with_delivery_service(
            group_id,
            &alice,
            &[(bob_pin, bob_package), (charlie_pin, charlie_package)],
            signer.identity(),
        )
        .unwrap();
        conversation.accept_pending_commit(&alice).unwrap();

        let proposal = signer.sign_remove(group_id, pending.epoch, 1).unwrap();
        assert!(matches!(
            conversation
                .process_external_remove_proposal(&alice, &proposal)
                .unwrap(),
            crate::ExternalProposalAction::Commit { .. }
        ));
    }

    #[test]
    fn signer_rejects_invalid_routing_bounds() {
        let signer = DeliveryServiceSigner::from_seed([0x24; DELIVERY_SERVICE_SEED_BYTES]).unwrap();
        let group_id = FilamentGroupId::new();
        assert_eq!(
            signer.sign_remove(group_id, 0, 0).unwrap_err(),
            ConversationError::LimitExceeded
        );
        assert_eq!(
            signer
                .sign_remove(group_id, 1, u32::try_from(MAX_MLS_GROUP_LEAVES).unwrap(),)
                .unwrap_err(),
            ConversationError::LimitExceeded
        );
    }
}
