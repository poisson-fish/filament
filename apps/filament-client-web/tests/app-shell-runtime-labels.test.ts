import { createSignal } from "solid-js";
import { describe, expect, it } from "vitest";
import { userIdFromInput } from "../src/domain/chat";
import { createAppShellRuntimeLabels } from "../src/features/app-shell/runtime/runtime-labels";

const USER_ID = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAZ");

describe("app shell runtime labels", () => {
  it("resolves actor labels from voice identities and username cache", () => {
    const [resolvedUsernames] = createSignal<Record<string, string>>({
      [USER_ID]: "owner",
    });
    const labels = createAppShellRuntimeLabels({ resolvedUsernames });

    expect(labels.actorLookupId(`u.${USER_ID}.mic`)).toBe(USER_ID);
    expect(labels.actorLabel(`u.${USER_ID}.mic`)).toBe("owner");
    expect(labels.displayUserLabel(USER_ID)).toBe("owner");
  });

  it("falls back to shortened actor IDs and local participant suffix", () => {
    const [resolvedUsernames] = createSignal<Record<string, string>>({});
    const labels = createAppShellRuntimeLabels({ resolvedUsernames });

    expect(labels.actorLabel("this-is-a-very-long-actor-id")).toBe("this-is-a-very...");
    expect(labels.voiceParticipantLabel("local-user", true)).toBe("local-user (you)");
    expect(labels.voiceParticipantLabel("remote-user", false)).toBe("remote-user");
  });
});
