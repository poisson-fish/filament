import { createEffect, onCleanup, Show } from "solid-js";
import type { RtcVideoTrackSnapshot } from "../../../../lib/rtc";

export interface VideoTileProps {
    trackSnapshot: RtcVideoTrackSnapshot;
    userIdFromVoiceIdentity: (identity: string) => string | null;
    actorLabel: (id: string) => string;
    resolveActorNameColor?: (id: string) => string | null;
    resolveAvatarUrl: (userId: string) => string | null | undefined;
    attachVideoTrack: (trackSid: string, element: HTMLVideoElement) => void;
    detachVideoTrack: (trackSid: string, element: HTMLVideoElement) => void;
}

export function VideoTile(props: VideoTileProps) {
    let videoRef!: HTMLVideoElement;

    const resolvedUserId = () => props.userIdFromVoiceIdentity(props.trackSnapshot.participantIdentity);
    const labelId = () => resolvedUserId() ?? props.trackSnapshot.participantIdentity;
    const label = () => props.actorLabel(labelId());
    const labelColor = () => props.resolveActorNameColor?.(labelId()) ?? null;
    const avatarUrl = () => {
        const id = resolvedUserId();
        return id ? props.resolveAvatarUrl(id) : undefined;
    };

    createEffect(() => {
        const trackSid = props.trackSnapshot.trackSid;
        if (videoRef && trackSid) {
            props.attachVideoTrack(trackSid, videoRef);
            onCleanup(() => {
                props.detachVideoTrack(trackSid, videoRef);
            });
        }
    });

    return (
        <div class="relative w-full h-full bg-black rounded-lg overflow-hidden flex items-center justify-center border border-solid border-[var(--line)]">
            <video
                ref={videoRef}
                autoplay
                playsinline
                muted
                class="w-full h-full object-contain"
            />

            <div class="absolute bottom-[0.8rem] left-[0.8rem] bg-black/60 backdrop-blur-[4px] px-[0.6rem] py-[0.4rem] rounded-[0.4rem] text-white flex items-center gap-2 text-[0.85rem] font-medium">
                <span class="relative inline-flex h-[1.2rem] w-[1.2rem] flex-shrink-0 items-center justify-center overflow-hidden rounded-full border border-[var(--line)] bg-[var(--bg-3)] text-[0.45rem] font-[800] tracking-[0.03em] uppercase">
                    <span class="z-[1]" aria-hidden="true">
                        {props.actorLabel(labelId())[0]}
                    </span>
                    <Show when={avatarUrl()}>
                        {(url) => <img src={url()} alt={`${label()} avatar`} class="absolute inset-0 z-[2] h-full w-full rounded-[inherit] object-cover" onError={(e) => { e.currentTarget.style.display = "none"; }} />}
                    </Show>
                </span>
                <span style={labelColor() ? { color: labelColor()! } : undefined}>{label()}</span>
                <Show when={props.trackSnapshot.source === "screen_share"}>
                    <span class="bg-white/20 px-[0.35rem] py-[0.1rem] rounded-[0.2rem] text-[0.70rem] uppercase tracking-[0.05em] opacity-90">Screen</span>
                </Show>
            </div>
        </div>
    );
}
