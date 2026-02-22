import { describe, expect, it } from "vitest";
import {
  DEFAULT_SETTINGS_CATEGORY,
  DEFAULT_VOICE_SETTINGS_SUBMENU,
  SETTINGS_CATEGORIES,
  VOICE_SETTINGS_SUBMENU,
} from "../src/features/app-shell/config/settings-menu";

describe("app shell config", () => {
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
