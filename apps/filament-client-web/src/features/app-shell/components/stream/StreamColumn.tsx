import { For, Show } from "solid-js";
import type { RtcSnapshot } from "../../../../lib/rtc";
import { VideoTile } from "./VideoTile";
import { StreamControls } from "./StreamControls";

export interface StreamColumnProps {
    rtcSnapshot: RtcSnapshot;
    userIdFromVoiceIdentity: (identity: string) => string | null;
    actorLabel: (id: string) => string;
    resolveActorNameColor?: (id: string) => string | null;
    resolveAvatarUrl: (userId: string) => string | null | undefined;
    attachVideoTrack: (trackSid: string, element: HTMLVideoElement) => void;
    detachVideoTrack: (trackSid: string, element: HTMLVideoElement) => void;

    canToggleVoiceCamera: boolean;
    canToggleVoiceScreenShare: boolean;
    isJoiningVoice: boolean;
    isLeavingVoice: boolean;
    isTogglingVoiceMic: boolean;
    isTogglingVoiceDeaf: boolean;
    isTogglingVoiceCamera: boolean;
    isTogglingVoiceScreenShare: boolean;

    onToggleVoiceMicrophone: () => void;
    onToggleVoiceDeafen: () => void;
    onToggleVoiceCamera: () => void;
    onToggleVoiceScreenShare: () => void;
    onLeaveVoice: () => void;
}

export function StreamColumn(props: StreamColumnProps) {
    const gridClass = () => {
        const count = props.rtcSnapshot.videoTracks.length;
        if (count === 0) return "";
        if (count === 1) return "grid-cols-1 grid-rows-1";
        if (count <= 2) return "grid-cols-2 grid-rows-1";
        if (count <= 4) return "grid-cols-2 grid-rows-2";
        return "grid-cols-[repeat(auto-fit,minmax(280px,1fr))] auto-rows-[minmax(200px,1fr)]";
    };

    return (
        <div class="flex flex-col w-full h-full bg-[var(--bg-0)] border-r border-solid border-[var(--line)] overflow-hidden">
            <div class={`flex-1 grid gap-4 p-4 overflow-y-auto items-center justify-center ${gridClass()}`}>
                <Show when={props.rtcSnapshot.videoTracks.length === 0}>
                    <div class="flex items-center justify-center h-full w-full text-[var(--ink-2)] text-[0.95rem] text-center">
                        <p>Ready to stream. Turn on your camera or share your screen.</p>
                    </div>
                </Show>
                <For each={props.rtcSnapshot.videoTracks}>
                    {(trackSnapshot) => (
                        <VideoTile
                            trackSnapshot={trackSnapshot}
                            userIdFromVoiceIdentity={props.userIdFromVoiceIdentity}
                            actorLabel={props.actorLabel}
                            resolveActorNameColor={props.resolveActorNameColor}
                            resolveAvatarUrl={props.resolveAvatarUrl}
                            attachVideoTrack={props.attachVideoTrack}
                            detachVideoTrack={props.detachVideoTrack}
                        />
                    )}
                </For>
            </div>

            <StreamControls
                rtcSnapshot={props.rtcSnapshot}
                canToggleVoiceCamera={props.canToggleVoiceCamera}
                canToggleVoiceScreenShare={props.canToggleVoiceScreenShare}
                isJoiningVoice={props.isJoiningVoice}
                isLeavingVoice={props.isLeavingVoice}
                isTogglingVoiceMic={props.isTogglingVoiceMic}
                isTogglingVoiceDeaf={props.isTogglingVoiceDeaf}
                isTogglingVoiceCamera={props.isTogglingVoiceCamera}
                isTogglingVoiceScreenShare={props.isTogglingVoiceScreenShare}
                onToggleVoiceMicrophone={props.onToggleVoiceMicrophone}
                onToggleVoiceDeafen={props.onToggleVoiceDeafen}
                onToggleVoiceCamera={props.onToggleVoiceCamera}
                onToggleVoiceScreenShare={props.onToggleVoiceScreenShare}
                onLeaveVoice={props.onLeaveVoice}
            />
        </div>
    );
}
