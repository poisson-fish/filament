import { createSignal } from "solid-js";
import { describe, expect, it } from "vitest";
import {
  channelIdFromInput,
  guildIdFromInput,
} from "../src/domain/chat";
import { createWorkspaceSelectionActions } from "../src/features/app-shell/runtime/workspace-selection-actions";

describe("app shell workspace selection actions", () => {
  it("opens text channel create panel with text kind", () => {
    const [newChannelKind, setNewChannelKind] = createSignal<"text" | "voice">("voice");
    const [activeOverlayPanel, setActiveOverlayPanel] = createSignal<string | null>(null);

    const actions = createWorkspaceSelectionActions({
      setNewChannelKind,
      openOverlayPanel: setActiveOverlayPanel,
      setActiveGuildId: () => guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAA"),
      setActiveChannelId: () => null,
    });

    actions.openTextChannelCreatePanel();

    expect(newChannelKind()).toBe("text");
    expect(activeOverlayPanel()).toBe("channel-create");
  });

  it("opens voice channel create panel with voice kind", () => {
    const [newChannelKind, setNewChannelKind] = createSignal<"text" | "voice">("text");
    const [activeOverlayPanel, setActiveOverlayPanel] = createSignal<string | null>(null);

    const actions = createWorkspaceSelectionActions({
      setNewChannelKind,
      openOverlayPanel: setActiveOverlayPanel,
      setActiveGuildId: () => guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAB"),
      setActiveChannelId: () => null,
    });

    actions.openVoiceChannelCreatePanel();

    expect(newChannelKind()).toBe("voice");
    expect(activeOverlayPanel()).toBe("channel-create");
  });

  it("selects workspace and optional first channel", () => {
    const [activeGuildId, setActiveGuildId] = createSignal<ReturnType<typeof guildIdFromInput>>(
      guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAC"),
    );
    const [activeChannelId, setActiveChannelId] = createSignal<
      ReturnType<typeof channelIdFromInput> | null
    >(channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAD"));

    const actions = createWorkspaceSelectionActions({
      setNewChannelKind: () => "text",
      openOverlayPanel: () => undefined,
      setActiveGuildId,
      setActiveChannelId,
    });

    const nextGuildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAE");
    const nextChannelId = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAF");

    actions.onSelectWorkspace(nextGuildId, nextChannelId);
    expect(activeGuildId()).toBe(nextGuildId);
    expect(activeChannelId()).toBe(nextChannelId);

    actions.onSelectWorkspace(nextGuildId, null);
    expect(activeChannelId()).toBeNull();
  });
});