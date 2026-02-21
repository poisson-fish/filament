import { Show } from "solid-js";
import type { RtcSnapshot } from "../../../../lib/rtc";

const MUTE_MIC_ICON_URL = new URL("../../../../resource/coolicons.v4.1/cooliocns SVG/Media/Volume_Off.svg", import.meta.url).href;
const UNMUTE_MIC_ICON_URL = new URL("../../../../resource/coolicons.v4.1/cooliocns SVG/Media/Volume_Max.svg", import.meta.url).href;
const HEADPHONES_ICON_URL = new URL("../../../../resource/coolicons.v4.1/cooliocns SVG/Media/Headphones.svg", import.meta.url).href;
const CAMERA_ICON_URL = new URL("../../../../resource/coolicons.v4.1/cooliocns SVG/System/Camera.svg", import.meta.url).href;
const START_SCREEN_SHARE_ICON_URL = new URL("../../../../resource/coolicons.v4.1/cooliocns SVG/System/Monitor_Play.svg", import.meta.url).href;
const STOP_SCREEN_SHARE_ICON_URL = new URL("../../../../resource/coolicons.v4.1/cooliocns SVG/System/Monitor.svg", import.meta.url).href;
const LEAVE_VOICE_ICON_URL = new URL("../../../../resource/coolicons.v4.1/cooliocns SVG/Interface/Log_Out.svg", import.meta.url).href;

export interface StreamControlsProps {
    rtcSnapshot: RtcSnapshot;
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

export function StreamControls(props: StreamControlsProps) {
    const isConnected = () => props.rtcSnapshot.connectionStatus === "connected";
    const disableControls = () => !isConnected() || props.isJoiningVoice || props.isLeavingVoice;

    const baseBtn = "inline-flex items-center justify-center w-[3.2rem] h-[3.2rem] rounded-full border-none transition-all duration-200 ease text-[var(--ink-0)] disabled:opacity-50 disabled:cursor-not-allowed enabled:hover:bg-[var(--bg-4)] enabled:hover:-translate-y-[2px]";
    const dangerBtn = "inline-flex items-center justify-center w-[3.2rem] h-[3.2rem] rounded-full border-none transition-all duration-200 ease disabled:opacity-50 disabled:cursor-not-allowed bg-[var(--danger-panel)] text-[var(--danger-ink)] enabled:hover:bg-[var(--danger)] enabled:hover:text-white enabled:hover:-translate-y-[2px]";

    const iconClass = "w-[1.4rem] h-[1.4rem] bg-current [mask-image:var(--icon-url)] [mask-size:contain] [mask-repeat:no-repeat] [mask-position:center] [-webkit-mask-image:var(--icon-url)] [-webkit-mask-size:contain] [-webkit-mask-repeat:no-repeat] [-webkit-mask-position:center]";

    const btnClasses = (isActive: boolean, isPulsing: boolean) => ({
        "bg-[var(--bg-0)]": isActive,
        "bg-[var(--bg-3)]": !isActive,
        "animate-pulse": isPulsing
    });

    return (
        <div class="flex justify-center items-center gap-3 p-4 bg-[var(--bg-1)] border-t border-solid border-[var(--line)]">
            <button
                type="button"
                class={baseBtn}
                classList={btnClasses(props.rtcSnapshot.isMicrophoneEnabled, props.isTogglingVoiceMic)}
                aria-label={props.rtcSnapshot.isMicrophoneEnabled ? "Mute Mic" : "Unmute Mic"}
                title={props.rtcSnapshot.isMicrophoneEnabled ? "Mute Mic" : "Unmute Mic"}
                onClick={props.onToggleVoiceMicrophone}
                disabled={disableControls() || props.isTogglingVoiceMic}
            >
                <span
                    class={iconClass}
                    style={`--icon-url: url("${props.rtcSnapshot.isMicrophoneEnabled ? UNMUTE_MIC_ICON_URL : MUTE_MIC_ICON_URL}")`}
                />
            </button>

            <button
                type="button"
                class={baseBtn}
                classList={btnClasses(props.rtcSnapshot.isCameraEnabled, props.isTogglingVoiceCamera)}
                aria-label={props.rtcSnapshot.isCameraEnabled ? "Camera Off" : "Camera On"}
                title={props.rtcSnapshot.isCameraEnabled ? "Camera Off" : "Camera On"}
                onClick={props.onToggleVoiceCamera}
                disabled={disableControls() || props.isTogglingVoiceCamera || !props.canToggleVoiceCamera}
            >
                <span
                    class={iconClass}
                    style={`--icon-url: url("${CAMERA_ICON_URL}")`}
                />
            </button>

            <button
                type="button"
                class={baseBtn}
                classList={btnClasses(props.rtcSnapshot.isScreenShareEnabled, props.isTogglingVoiceScreenShare)}
                aria-label={props.rtcSnapshot.isScreenShareEnabled ? "Stop Share" : "Share Screen"}
                title={props.rtcSnapshot.isScreenShareEnabled ? "Stop Share" : "Share Screen"}
                onClick={props.onToggleVoiceScreenShare}
                disabled={disableControls() || props.isTogglingVoiceScreenShare || !props.canToggleVoiceScreenShare}
            >
                <span
                    class={iconClass}
                    style={`--icon-url: url("${props.rtcSnapshot.isScreenShareEnabled ? STOP_SCREEN_SHARE_ICON_URL : START_SCREEN_SHARE_ICON_URL}")`}
                />
            </button>

            <button
                type="button"
                class={baseBtn}
                classList={btnClasses(props.rtcSnapshot.isDeafened, props.isTogglingVoiceDeaf)}
                aria-label={props.rtcSnapshot.isDeafened ? "Undeafen Audio" : "Deafen Audio"}
                title={props.rtcSnapshot.isDeafened ? "Undeafen Audio" : "Deafen Audio"}
                onClick={props.onToggleVoiceDeafen}
                disabled={disableControls() || props.isTogglingVoiceDeaf}
            >
                <span
                    class={iconClass}
                    style={`--icon-url: url("${HEADPHONES_ICON_URL}")`}
                />
            </button>

            <button
                type="button"
                class={dangerBtn}
                aria-label="Disconnect"
                title="Disconnect"
                onClick={props.onLeaveVoice}
                disabled={props.isLeavingVoice || props.isJoiningVoice}
            >
                <span
                    class={iconClass}
                    style={`--icon-url: url("${LEAVE_VOICE_ICON_URL}")`}
                />
            </button>
        </div>
    );
}
