import { describe, expect, it } from "vitest";
import { OPENMOJI_REACTION_OPTIONS } from "../src/features/app-shell/config/reaction-options";
import {
  DEFAULT_SETTINGS_CATEGORY,
  DEFAULT_VOICE_SETTINGS_SUBMENU,
  SETTINGS_CATEGORIES,
  VOICE_SETTINGS_SUBMENU,
} from "../src/features/app-shell/config/settings-menu";

describe("app shell config", () => {
  it("keeps reaction options labels non-empty and emoji IDs unique", () => {
    expect(OPENMOJI_REACTION_OPTIONS.length).toBeGreaterThan(0);

    const emojis = new Set<string>();
    for (const option of OPENMOJI_REACTION_OPTIONS) {
      expect(option.label.trim().length).toBeGreaterThan(0);
      expect(option.iconUrl.length).toBeGreaterThan(0);
      emojis.add(option.emoji);
    }

    expect(emojis.size).toBe(OPENMOJI_REACTION_OPTIONS.length);
  });

  it("keeps settings menu IDs unique and includes default selections", () => {
    expect(SETTINGS_CATEGORIES.length).toBeGreaterThan(0);
    expect(VOICE_SETTINGS_SUBMENU.length).toBeGreaterThan(0);

    const categoryIds = SETTINGS_CATEGORIES.map((item) => item.id);
    expect(new Set(categoryIds).size).toBe(categoryIds.length);
    expect(categoryIds).toContain(DEFAULT_SETTINGS_CATEGORY);

    const submenuIds = VOICE_SETTINGS_SUBMENU.map((item) => item.id);
    expect(new Set(submenuIds).size).toBe(submenuIds.length);
    expect(submenuIds).toContain(DEFAULT_VOICE_SETTINGS_SUBMENU);
  });
});
