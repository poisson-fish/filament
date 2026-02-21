use std::collections::HashMap;
use tokio::time::{interval, Duration};

use crate::server::{
    auth::now_unix,
    core::AppState,
    realtime::{
        remove_voice_participant_for_channel, update_voice_participant_audio_state_for_channel,
    },
};

pub(crate) async fn start_livekit_sync(state: AppState) {
    let room_client = match &state.livekit_room {
        Some(client) => client,
        None => return,
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
            let lk_participants = match room_client.list_participants(&room_name).await {
                Ok(participants) => participants,
                Err(_) => continue, // Log if needed
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
                    remove_voice_participant_for_channel(&state, *user_id, guild_id, channel_id, now)
                        .await;
                }
            }

            // 2. Zombie Ghosting
            // In LiveKit, not in Filament -> Kicked/Banned!
            for (lk_identity, _p) in &lk_identities {
                let found_in_filament = filament_participants.values().any(|fp| &fp.identity == lk_identity);
                if !found_in_filament {
                    let _ = room_client.remove_participant(&room_name, lk_identity).await;
                }
            }
        }
    }
}
