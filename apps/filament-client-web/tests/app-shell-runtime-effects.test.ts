import { createRoot, createSignal } from "solid-js";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { workspaceFromStorage } from "../src/domain/chat";
import { createRuntimeEffects } from "../src/features/app-shell/runtime/runtime-effects";

vi.mock("../src/lib/workspace-cache", () => ({
  saveWorkspaceCache: vi.fn(),
}));

import { saveWorkspaceCache } from "../src/lib/workspace-cache";

function flushEffects(): Promise<void> {
  return new Promise((resolve) => {
    queueMicrotask(() => resolve());
  });
}

const SAMPLE_WORKSPACE = workspaceFromStorage({
  guildId: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
  guildName: "Security Ops",
  visibility: "private",
  channels: [],
});

describe("app shell runtime effects", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("persists workspace cache only after bootstrap completes", async () => {
    await createRoot(async (dispose) => {
      const [workspaceBootstrapDone, setWorkspaceBootstrapDone] = createSignal(false);
      const [workspaces, setWorkspaces] = createSignal([SAMPLE_WORKSPACE]);
      const [activeOverlayPanel, setActiveOverlayPanel] = createSignal<
        "workspace-create" | "client-settings" | null
      >(null);
      const [activeSettingsCategory] = createSignal<"voice" | "profile">("profile");
      const [activeVoiceSettingsSubmenu] = createSignal<"audio-devices">("audio-devices");
      const refreshAudioDeviceInventory = vi.fn(async () => undefined);

      createRuntimeEffects({
        workspaceBootstrapDone,
        workspaces,
        setActiveOverlayPanel,
        activeOverlayPanel,
        activeSettingsCategory,
        activeVoiceSettingsSubmenu,
        refreshAudioDeviceInventory,
      });

      await flushEffects();

      expect(saveWorkspaceCache).not.toHaveBeenCalled();

      setWorkspaceBootstrapDone(true);
      await flushEffects();

      expect(saveWorkspaceCache).toHaveBeenCalledWith([SAMPLE_WORKSPACE]);

      setWorkspaces([]);
      await flushEffects();

      expect(saveWorkspaceCache).toHaveBeenLastCalledWith([]);

      dispose();
    });
  });

  it("opens workspace create overlay when bootstrap completes with zero workspaces", async () => {
    await createRoot(async (dispose) => {
      const [workspaceBootstrapDone, setWorkspaceBootstrapDone] = createSignal(false);
      const [workspaces] = createSignal<typeof SAMPLE_WORKSPACE[]>([]);
      const [activeOverlayPanel, setActiveOverlayPanel] = createSignal<
        "workspace-create" | "client-settings" | null
      >(null);
      const [activeSettingsCategory] = createSignal<"voice" | "profile">("profile");
      const [activeVoiceSettingsSubmenu] = createSignal<"audio-devices">("audio-devices");

      createRuntimeEffects({
        workspaceBootstrapDone,
        workspaces,
        setActiveOverlayPanel,
        activeOverlayPanel,
        activeSettingsCategory,
        activeVoiceSettingsSubmenu,
        refreshAudioDeviceInventory: async () => undefined,
      });

      setWorkspaceBootstrapDone(true);
      await flushEffects();

      expect(activeOverlayPanel()).toBe("workspace-create");

      dispose();
    });
  });

  it("does not force workspace create overlay when workspace list is non-empty", async () => {
    await createRoot(async (dispose) => {
      const [workspaceBootstrapDone, setWorkspaceBootstrapDone] = createSignal(false);
      const [workspaces] = createSignal([SAMPLE_WORKSPACE]);
      const [activeOverlayPanel, setActiveOverlayPanel] = createSignal<
        "workspace-create" | "client-settings" | null
      >(null);
      const [activeSettingsCategory] = createSignal<"voice" | "profile">("profile");
      const [activeVoiceSettingsSubmenu] = createSignal<"audio-devices">("audio-devices");

      createRuntimeEffects({
        workspaceBootstrapDone,
        workspaces,
        setActiveOverlayPanel,
        activeOverlayPanel,
        activeSettingsCategory,
        activeVoiceSettingsSubmenu,
        refreshAudioDeviceInventory: async () => undefined,
      });

      setWorkspaceBootstrapDone(true);
      await flushEffects();

      expect(activeOverlayPanel()).toBeNull();

      dispose();
    });
  });

  it("refreshes voice devices when client voice audio settings panel opens", async () => {
    await createRoot(async (dispose) => {
      const [workspaceBootstrapDone] = createSignal(false);
      const [workspaces] = createSignal([SAMPLE_WORKSPACE]);
      const [activeOverlayPanel, setActiveOverlayPanel] = createSignal<
        "workspace-create" | "client-settings" | null
      >(null);
      const [activeSettingsCategory, setActiveSettingsCategory] =
        createSignal<"voice" | "profile">("profile");
      const [activeVoiceSettingsSubmenu] = createSignal<"audio-devices">("audio-devices");
      const refreshAudioDeviceInventory = vi.fn(async () => undefined);

      createRuntimeEffects({
        workspaceBootstrapDone,
        workspaces,
        setActiveOverlayPanel,
        activeOverlayPanel,
        activeSettingsCategory,
        activeVoiceSettingsSubmenu,
        refreshAudioDeviceInventory,
      });

      await flushEffects();

      expect(refreshAudioDeviceInventory).not.toHaveBeenCalled();

      setActiveOverlayPanel("client-settings");
      await flushEffects();
      expect(refreshAudioDeviceInventory).not.toHaveBeenCalled();

      setActiveSettingsCategory("voice");
      await flushEffects();
      expect(refreshAudioDeviceInventory).toHaveBeenCalledTimes(1);
      expect(refreshAudioDeviceInventory).toHaveBeenCalledWith(false);

      dispose();
    });
  });
});