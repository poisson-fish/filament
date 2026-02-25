use std::collections::{HashMap, HashSet};
use tokio::time::{interval, Duration};

use crate::server::{
    auth::now_unix,
    core::AppState,
    errors::AuthFailure,
    realtime::{
        remove_voice_participant_for_channel, update_voice_participant_audio_state_for_channel,
    },
};

pub(crate) async fn start_livekit_sync(state: AppState) {
    let Some(room_client) = &state.livekit_room else {
        return;
    };

    let mut ticker = interval(Duration::from_secs(15));
    loop {
        ticker.tick().await;

        let active_rooms = {
            let vp = state.realtime_registry.voice_participants().read().await;
            vp.keys().cloned().collect::<Vec<String>>()
        };

        for key in active_rooms {
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() != 2 {
                continue;
            }
            let guild_id = parts[0];
            let channel_id = parts[1];
            let room_name = format!("filament.voice.{guild_id}.{channel_id}");

            // Fetch LiveKit participants
            let Ok(lk_participants) = room_client.list_participants(&room_name).await else {
                continue; // Log if needed
            };

            let mut lk_identities = HashMap::new();
            for p in &lk_participants {
                lk_identities.insert(p.identity.clone(), p.clone());
            }

            let filament_participants = {
                let vp = state.realtime_registry.voice_participants().read().await;
                vp.get(&key).cloned().unwrap_or_default()
            };

            let now = now_unix();

            // 1. Ghost Presence & State Spoofing
            for (user_id, p) in &filament_participants {
                if let Some(lk_p) = lk_identities.get(&p.identity) {
                    // Check spoofing: Filament muted, LiveKit not muted
                    let mut actually_unmuted = false;
                    for track in &lk_p.tracks {
                        if track.r#type == 0 /* Audio */ && !track.muted {
                            actually_unmuted = true;
                            break;
                        }
                    }

                    if p.is_muted && actually_unmuted {
                        // Forcibly un-mute in Filament
                        update_voice_participant_audio_state_for_channel(
                            &state,
                            *user_id,
                            guild_id,
                            channel_id,
                            Some(false),
                            None,
                            now,
                        )
                        .await;
                    }
                } else {
                    // Not in LiveKit, but in Filament -> Ghost Presence!
                    remove_voice_participant_for_channel(
                        &state, *user_id, guild_id, channel_id, now,
                    )
                    .await;
                }
            }

            // 2. Zombie Ghosting
            // In LiveKit, not in Filament -> Kicked/Banned!
            for lk_identity in lk_identities.keys() {
                let found_in_filament = filament_participants
                    .values()
                    .any(|fp| &fp.identity == lk_identity);
                if !found_in_filament {
                    let _ = room_client
                        .remove_participant(&room_name, lk_identity)
                        .await;
                }
            }
        }
    }
}

pub(crate) async fn reevaluate_livekit_permissions_for_guild(state: &AppState, guild_id: &str) {
    let users_to_check = active_voice_users_for_guild(state, guild_id).await;

    let now = now_unix();
    for (user_id, channel_id) in users_to_check {
        if user_lacks_subscribe_permission(state, user_id, guild_id, &channel_id).await {
            remove_voice_participant_for_channel(state, user_id, guild_id, &channel_id, now).await;
        }
    }
}

pub(crate) fn schedule_livekit_permission_reevaluation_for_guild(state: &AppState, guild_id: &str) {
    let state = state.clone();
    let guild_id = guild_id.to_owned();
    tokio::spawn(async move {
        reevaluate_livekit_permissions_for_guild(&state, &guild_id).await;
    });
}

async fn active_voice_users_for_guild(
    state: &AppState,
    guild_id: &str,
) -> Vec<(filament_core::UserId, String)> {
    let mut users_to_check = HashSet::new();
    {
        let vp = state.realtime_registry.voice_participants().read().await;
        for (key, channel_participants) in vp.iter() {
            let Some((entry_guild_id, channel_id)) = key.split_once(':') else {
                continue;
            };
            if entry_guild_id != guild_id {
                continue;
            }
            for user_id in channel_participants.keys() {
                users_to_check.insert((*user_id, channel_id.to_owned()));
            }
        }
    }
    users_to_check.into_iter().collect()
}

async fn user_lacks_subscribe_permission(
    state: &AppState,
    user_id: filament_core::UserId,
    guild_id: &str,
    channel_id: &str,
) -> bool {
    match crate::server::domain::channel_permission_snapshot(state, user_id, guild_id, channel_id)
        .await
    {
        Ok((_, permissions)) => !permissions.contains(filament_core::Permission::SubscribeStreams),
        Err(err) => {
            if !matches!(err, AuthFailure::Forbidden | AuthFailure::NotFound) {
                tracing::warn!(
                    event = "voice.permission_reevaluate",
                    guild_id = %guild_id,
                    channel_id = %channel_id,
                    user_id = %user_id,
                    error = ?err,
                    "permission snapshot failed; removing participant defensively",
                );
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::{ChannelKind, ChannelPermissionOverwrite, PermissionSet, Role, UserId};

    use super::reevaluate_livekit_permissions_for_guild;
    use crate::server::core::{
        AppConfig, AppState, ChannelRecord, GuildRecord, GuildVisibility, VoiceParticipant,
        VoiceStreamKind,
    };

    fn voice_participant(user_id: UserId, identity: &str) -> VoiceParticipant {
        VoiceParticipant {
            user_id,
            identity: identity.to_owned(),
            joined_at_unix: 1,
            updated_at_unix: 1,
            expires_at_unix: i64::MAX,
            is_muted: false,
            is_deafened: false,
            is_speaking: false,
            is_video_enabled: false,
            is_screen_share_enabled: false,
            published_streams: HashSet::from([VoiceStreamKind::Microphone]),
        }
    }

    async fn seed_guild_with_member_and_channel(
        state: &AppState,
        guild_id: &str,
        channel_id: &str,
        user_id: UserId,
        role_overrides: HashMap<Role, ChannelPermissionOverwrite>,
    ) {
        let mut guild = GuildRecord {
            name: String::from("livekit-reeval-test"),
            visibility: GuildVisibility::Private,
            created_by_user_id: user_id,
            default_join_role_id: None,
            members: HashMap::new(),
            banned_members: HashSet::new(),
            channels: HashMap::new(),
        };
        guild.members.insert(user_id, Role::Member);
        guild.channels.insert(
            channel_id.to_owned(),
            ChannelRecord {
                name: String::from("voice"),
                kind: ChannelKind::Voice,
                messages: Vec::new(),
                role_overrides,
            },
        );
        state
            .membership_store
            .guilds()
            .write()
            .await
            .insert(guild_id.to_owned(), guild);
    }

    #[tokio::test]
    async fn reevaluation_removes_participant_when_subscribe_is_denied() {
        let state = AppState::new(&AppConfig::default()).expect("state should initialize");
        let guild_id = "g-deny";
        let channel_id = "c-voice";
        let user_id = UserId::new();
        let mut role_overrides = HashMap::new();
        role_overrides.insert(
            Role::Member,
            ChannelPermissionOverwrite {
                allow: PermissionSet::empty(),
                deny: PermissionSet::from_bits(1 << 11),
            },
        );
        seed_guild_with_member_and_channel(&state, guild_id, channel_id, user_id, role_overrides)
            .await;

        state
            .realtime_registry
            .voice_participants()
            .write()
            .await
            .insert(
                format!("{guild_id}:{channel_id}"),
                HashMap::from([(user_id, voice_participant(user_id, "u.deny"))]),
            );

        reevaluate_livekit_permissions_for_guild(&state, guild_id).await;

        let voice = state.realtime_registry.voice_participants().read().await;
        assert!(
            !voice.contains_key(&format!("{guild_id}:{channel_id}")),
            "participant should be removed when subscribe is denied",
        );
    }

    #[tokio::test]
    async fn reevaluation_keeps_participant_with_valid_subscribe_permission() {
        let state = AppState::new(&AppConfig::default()).expect("state should initialize");
        let guild_id = "g-allow";
        let channel_id = "c-voice";
        let user_id = UserId::new();
        seed_guild_with_member_and_channel(&state, guild_id, channel_id, user_id, HashMap::new())
            .await;

        state
            .realtime_registry
            .voice_participants()
            .write()
            .await
            .insert(
                format!("{guild_id}:{channel_id}"),
                HashMap::from([(user_id, voice_participant(user_id, "u.allow"))]),
            );

        reevaluate_livekit_permissions_for_guild(&state, guild_id).await;

        let voice = state.realtime_registry.voice_participants().read().await;
        let participants = voice
            .get(&format!("{guild_id}:{channel_id}"))
            .expect("participants should still exist");
        assert!(
            participants.contains_key(&user_id),
            "participant should remain when subscribe is still allowed",
        );
    }

    #[tokio::test]
    async fn reevaluation_fails_closed_when_permission_lookup_errors() {
        let state = AppState::new(&AppConfig::default()).expect("state should initialize");
        let guild_id = "g-error";
        let user_id = UserId::new();
        let missing_channel_id = "c-missing";
        seed_guild_with_member_and_channel(&state, guild_id, "c-real", user_id, HashMap::new())
            .await;

        state
            .realtime_registry
            .voice_participants()
            .write()
            .await
            .insert(
                format!("{guild_id}:{missing_channel_id}"),
                HashMap::from([(user_id, voice_participant(user_id, "u.error"))]),
            );

        reevaluate_livekit_permissions_for_guild(&state, guild_id).await;

        let voice = state.realtime_registry.voice_participants().read().await;
        assert!(
            !voice.contains_key(&format!("{guild_id}:{missing_channel_id}")),
            "participant should be removed when permission lookup fails",
        );
    }
}
