//! E2EE MLS Round-Trip Spike — Phase 0 Engineering Spike
//!
//! This spike validates the OpenMLS API surface by demonstrating a full
//! 2-member MLS group lifecycle:
//!
//! 1. Alice creates a group.
//! 2. Alice generates a KeyPackage for Bob and adds Bob to the group via Welcome.
//! 3. Alice sends an application message; Bob decrypts it.
//! 4. Bob self-updates (rekeys); Alice processes the commit.
//! 5. Alice removes Bob (cryptographic eviction).
//! 6. Bob rejoins via external commit (recovery from desync).
//!
//! This is a standalone crate, not part of the filament workspace.
//! Build with: `cargo test`
//!
//! See `docs/adr/0001-e2ee-mls-openmls.md` for the protocol decision and
//! `plans/PLAN_E2EE.md` for the full design specification.

#![forbid(unsafe_code)]

use openmls::prelude::*;
use openmls_basic_credential::BasicCredential;
use openmls_rust_crypto::OpenMlsRustCrypto;

/// Ciphersuite: MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519 (0x0003).
const CIPHERSUITE: Ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_ED25519;

/// A test participant with their own crypto provider, credential, and signer.
struct Participant {
    name: &'static str,
    provider: OpenMlsRustCrypto,
    credential_with_key: CredentialWithKey,
    signer: SignatureKeyPair,
}

impl Participant {
    /// Create a new participant with a BasicCredential and Ed25519 signature key.
    fn new(name: &'static str) -> Self {
        let provider = OpenMlsRustCrypto::default();
        let credential = BasicCredential::new(name.as_bytes().to_vec());
        let signature_keys =
            SignatureKeyPair::new(CIPHERSUITE.signature_algorithm()).expect("failed to generate signature keypair");
        signature_keys
            .store(provider.storage())
            .expect("failed to store signature keys");

        let credential_with_key = CredentialWithKey {
            credential: credential.into(),
            signature_key: signature_keys.to_public_vec().into(),
        };

        Self {
            name,
            provider,
            credential_with_key,
            signer: signature_keys,
        }
    }

    /// Generate a KeyPackage for this participant (for others to add them).
    fn generate_key_package(&self) -> KeyPackageBundle {
        KeyPackage::builder()
            .build(
                CIPHERSUITE,
                &self.provider,
                &self.signer,
                self.credential_with_key.clone(),
            )
            .expect("failed to build KeyPackage")
    }
}

/// MLS group config for the spike.
fn mls_group_config() -> MlsGroupJoinConfig {
    MlsGroupJoinConfig::builder()
        .use_ratchet_tree_extension(true)
        .build()
}

/// Run the full 2-member MLS group lifecycle spike.
///
/// This function demonstrates:
/// 1. Group creation (Alice)
/// 2. Adding a member via KeyPackage + Welcome (Alice adds Bob)
/// 3. Application message send/receive (Alice → Bob)
/// 4. Self-update / rekey (Bob)
/// 5. Member removal / cryptographic eviction (Alice removes Bob)
/// 6. External-commit recovery (Bob rejoins)
fn run_spike() {
    // --- Setup participants ---
    let alice = Participant::new("alice");
    let bob = Participant::new("bob");
    println!("[spike] Participants created: alice, bob");

    // --- 1. Alice creates the group ---
    let group_config = MlsGroupJoinConfig::builder()
        .use_ratchet_tree_extension(true)
        .build();

    let mut alice_group = MlsGroup::builder()
        .padding_size(100)
        .ciphersuite(CIPHERSUITE)
        .use_ratchet_tree_extension(true)
        .build(
            &alice.provider,
            &alice.signer,
            alice.credential_with_key.clone(),
        )
        .expect("alice: failed to create group");

    println!(
        "[spike] Alice created group, epoch = {}",
        alice_group.epoch().as_u64()
    );

    // --- 2. Alice generates Bob's KeyPackage and adds Bob ---
    let bob_key_package = bob.generate_key_package();
    println!("[spike] Bob generated KeyPackage");

    let (_add_msg, welcome, _group_info) = alice_group
        .add_members(
            &alice.provider,
            &alice.signer,
            &[bob_key_package.key_package()],
        )
        .expect("alice: failed to add bob");

    alice_group
        .merge_pending_commit(&alice.provider)
        .expect("alice: failed to merge pending commit (add bob)");

    println!(
        "[spike] Alice added Bob, epoch = {}",
        alice_group.epoch().as_u64()
    );

    // --- Bob joins from Welcome ---
    let welcome = welcome.into_welcome().expect("failed to extract welcome");
    let staged_join = StagedWelcome::new_from_welcome(
        &bob.provider,
        &mls_group_config(),
        welcome,
        None, // No ratchet tree extension in this simple spike
    )
    .expect("bob: failed to stage welcome");

    let mut bob_group = staged_join
        .into_group(&bob.provider)
        .expect("bob: failed to join group from welcome");

    println!(
        "[spike] Bob joined group, epoch = {}",
        bob_group.epoch().as_u64()
    );

    // --- 3. Alice sends a message; Bob decrypts ---
    let plaintext = b"Hello Bob, this is Alice!";
    let alice_msg = alice_group
        .create_message(&alice.provider, &alice.signer, plaintext)
        .expect("alice: failed to create message");

    let alice_msg_bytes = alice_msg
        .to_bytes()
        .expect("alice: failed to serialize message");

    // Bob processes and decrypts
    let mls_message_in =
        MlsMessageIn::tls_deserialize_exact(&alice_msg_bytes[..]).expect("bob: failed to deserialize message");
    let protocol_message: ProtocolMessage = mls_message_in
        .try_into_protocol_message()
        .expect("bob: failed to convert to protocol message");

    let processed = bob_group
        .process_message(&bob.provider, protocol_message)
        .expect("bob: failed to process message");

    let decrypted = match processed.into_content() {
        ProcessedMessageContent::ApplicationMessage(app_msg) => {
            app_msg.into_bytes()
        }
        _ => panic!("bob: expected application message, got something else"),
    };

    assert_eq!(decrypted.as_slice(), plaintext);
    println!("[spike] Alice sent message, Bob decrypted: {:?}", String::from_utf8_lossy(&decrypted));

    // --- 4. Bob self-updates (rekeys) ---
    let (update_msg, _welcome_option) = bob_group
        .self_update(&bob.provider, &bob.signer)
        .expect("bob: failed to self-update");

    bob_group
        .merge_pending_commit(&bob.provider)
        .expect("bob: failed to merge pending commit (self-update)");

    println!(
        "[spike] Bob self-updated, epoch = {}",
        bob_group.epoch().as_u64()
    );

    // Alice processes Bob's update commit
    let update_bytes = update_msg
        .to_bytes()
        .expect("alice: failed to serialize update message");
    let update_in = MlsMessageIn::tls_deserialize_exact(&update_bytes[..])
        .expect("alice: failed to deserialize update message");
    let update_protocol: ProtocolMessage = update_in
        .try_into_protocol_message()
        .expect("alice: failed to convert update to protocol message");

    let processed_update = alice_group
        .process_message(&alice.provider, update_protocol)
        .expect("alice: failed to process bob's update");

    match processed_update.into_content() {
        ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
            alice_group
                .merge_staged_commit(&alice.provider, *staged_commit)
                .expect("alice: failed to merge staged commit (bob's update)");
        }
        _ => panic!("alice: expected staged commit from bob's update"),
    }

    println!(
        "[spike] Alice processed Bob's update, epoch = {}",
        alice_group.epoch().as_u64()
    );

    // --- 5. Alice removes Bob (cryptographic eviction) ---
    // Get Bob's leaf index from Alice's group
    let bob_leaf = alice_group
        .members()
        .find(|m| {
            // Match by checking the credential — Bob's credential contains "bob"
            m.credential
                .serialize()
                .map(|bytes| bytes.windows(3).any(|w| w == b"bob"))
                .unwrap_or(false)
        })
        .expect("alice: bob not found in group members");

    let (remove_msg, _welcome_option) = alice_group
        .remove_members(&alice.provider, &alice.signer, &[bob_leaf.index])
        .expect("alice: failed to remove bob");

    alice_group
        .merge_pending_commit(&alice.provider)
        .expect("alice: failed to merge pending commit (remove bob)");

    println!(
        "[spike] Alice removed Bob, epoch = {}",
        alice_group.epoch().as_u64()
    );

    // Bob processes his own removal
    let remove_bytes = remove_msg
        .to_bytes()
        .expect("bob: failed to serialize remove message");
    let remove_in = MlsMessageIn::tls_deserialize_exact(&remove_bytes[..])
        .expect("bob: failed to deserialize remove message");
    let remove_protocol: ProtocolMessage = remove_in
        .try_into_protocol_message()
        .expect("bob: failed to convert remove to protocol message");

    let processed_remove = bob_group
        .process_message(&bob.provider, remove_protocol)
        .expect("bob: failed to process removal");

    match processed_remove.into_content() {
        ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
            bob_group
                .merge_staged_commit(&bob.provider, *staged_commit)
                .expect("bob: failed to merge staged commit (own removal)");
        }
        _ => panic!("bob: expected staged commit from removal"),
    }

    println!("[spike] Bob processed his own removal (cryptographic eviction)");

    // --- 6. Bob rejoins via external commit (recovery from desync) ---
    // Alice exports GroupInfo for Bob's recovery
    let group_info = alice_group
        .export_group_info(&alice.provider, &alice.signer, true)
        .expect("alice: failed to export group info");

    let verifiable_group_info = group_info
        .into_verifiable_group_info()
        .expect("failed to convert group info to verifiable");

    // Bob builds a new group via external commit
    let (mut bob_new_group, _bundle) = MlsGroup::external_commit_builder()
        .with_config(mls_group_config())
        .build_group(
            &bob.provider,
            verifiable_group_info,
            bob.credential_with_key.clone(),
        )
        .expect("bob: failed to build external commit group")
        .build(
            bob.provider.rand(),
            bob.provider.crypto(),
            &bob.signer,
            |_| true,
        )
        .expect("bob: failed to build external commit")
        .finalize(&bob.provider)
        .expect("bob: failed to finalize external commit");

    bob_new_group
        .merge_pending_commit(&bob.provider)
        .expect("bob: failed to merge pending commit (external rejoin)");

    println!(
        "[spike] Bob rejoined via external commit, epoch = {}",
        bob_new_group.epoch().as_u64()
    );

    // --- Verify: Alice sends a message, Bob (rejoined) can decrypt ---
    let plaintext2 = b"Welcome back Bob!";
    let alice_msg2 = alice_group
        .create_message(&alice.provider, &alice.signer, plaintext2)
        .expect("alice: failed to create second message");

    let alice_msg2_bytes = alice_msg2
        .to_bytes()
        .expect("alice: failed to serialize second message");

    let mls_message_in2 =
        MlsMessageIn::tls_deserialize_exact(&alice_msg2_bytes[..]).expect("bob: failed to deserialize second message");
    let protocol_message2: ProtocolMessage = mls_message_in2
        .try_into_protocol_message()
        .expect("bob: failed to convert second message to protocol message");

    let processed2 = bob_new_group
        .process_message(&bob.provider, protocol_message2)
        .expect("bob: failed to process second message");

    let decrypted2 = match processed2.into_content() {
        ProcessedMessageContent::ApplicationMessage(app_msg) => app_msg.into_bytes(),
        _ => panic!("bob: expected application message after rejoin, got something else"),
    };

    assert_eq!(decrypted2.as_slice(), plaintext2);
    println!(
        "[spike] After rejoin, Alice sent message, Bob decrypted: {:?}",
        String::from_utf8_lossy(&decrypted2)
    );

    // --- Verify: exporter secret derivation (for SFrame in Phase 5) ---
    let alice_secret = alice_group
        .export_secret(alice.provider.crypto(), "filament_media_v1", &[], 32)
        .expect("alice: failed to export secret");

    let bob_secret = bob_new_group
        .export_secret(bob.provider.crypto(), "filament_media_v1", &[], 32)
        .expect("bob: failed to export secret");

    assert_eq!(alice_secret.as_slice(), bob_secret.as_slice());
    println!("[spike] Exporter secrets match between Alice and Bob (32 bytes)");

    println!("[spike] Full 2-member MLS lifecycle completed successfully!");
}

fn main() {
    run_spike();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_2_member_mls_lifecycle() {
        run_spike();
    }
}
