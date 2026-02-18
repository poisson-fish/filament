// @vitest-environment node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const testDir = resolve(fileURLToPath(new URL(".", import.meta.url)));
const webRootDir = resolve(testDir, "..");
const appCssPath = resolve(webRootDir, "src/styles/app.css");
const tokensCssPath = resolve(webRootDir, "src/styles/app/tokens.css");
const baseCssPath = resolve(webRootDir, "src/styles/app/base.css");
const shellRefreshCssPath = resolve(webRootDir, "src/styles/app/shell-refresh.css");
const channelRailPath = resolve(
  webRootDir,
  "src/features/app-shell/components/ChannelRail.tsx",
);
const serverRailPath = resolve(
  webRootDir,
  "src/features/app-shell/components/ServerRail.tsx",
);
const messageComposerPath = resolve(
  webRootDir,
  "src/features/app-shell/components/messages/MessageComposer.tsx",
);
const messageRowPath = resolve(
  webRootDir,
  "src/features/app-shell/components/messages/MessageRow.tsx",
);
const reactionPickerPortalPath = resolve(
  webRootDir,
  "src/features/app-shell/components/messages/ReactionPickerPortal.tsx",
);
const migratedTsxPaths = [
  channelRailPath,
  serverRailPath,
  messageComposerPath,
  messageRowPath,
  reactionPickerPortalPath,
];

const rawColorLiteralPattern = /#[0-9a-f]{3,8}\b|\b(?:rgb|rgba|hsl|hsla)\(/i;

function parseImportPaths(cssSource: string): string[] {
  return [...cssSource.matchAll(/@import\s+"([^"]+)";/g)].map((match) => match[1]);
}

describe("app style token manifest", () => {
  it("keeps tokens.css first in the stylesheet manifest order", () => {
    const appCss = readFileSync(appCssPath, "utf8");
    expect(parseImportPaths(appCss)).toEqual([
      "./app/tokens.css",
      "./app/base.css",
      "./app/shell-refresh.css",
    ]);
  });

  it("defines canonical palette tokens in tokens.css", () => {
    const tokensCss = readFileSync(tokensCssPath, "utf8");
    const requiredTokenNames = [
      "--bg-0",
      "--bg-1",
      "--bg-2",
      "--bg-3",
      "--bg-4",
      "--panel",
      "--panel-soft",
      "--ink-0",
      "--ink-1",
      "--ink-2",
      "--line",
      "--line-soft",
      "--brand",
      "--brand-strong",
      "--danger",
      "--ok",
      "--shadow",
      "--font-main",
      "--font-code",
      "--danger-panel",
      "--danger-panel-strong",
      "--danger-ink",
    ];

    for (const tokenName of requiredTokenNames) {
      expect(tokensCss).toMatch(new RegExp(`${tokenName}\\s*:`));
    }
  });

  it("avoids re-declaring canonical palette tokens in base.css", () => {
    const baseCss = readFileSync(baseCssPath, "utf8");
    expect(baseCss).not.toMatch(/--bg-0\s*:/);
    expect(baseCss).not.toMatch(/--ink-0\s*:/);
    expect(baseCss).not.toMatch(/--line\s*:/);
  });

  it("removes legacy reaction picker selectors from base.css", () => {
    const baseCss = readFileSync(baseCssPath, "utf8");
    const removedSelectors = [
      ".reaction-picker {",
      ".reaction-picker-floating {",
      ".reaction-picker-header {",
      ".reaction-picker-title {",
      ".reaction-picker-close {",
      ".reaction-picker-grid {",
      ".reaction-picker-option {",
      ".reaction-picker-option:hover {",
      ".reaction-picker-option img {",
    ];

    for (const selector of removedSelectors) {
      expect(baseCss).not.toContain(selector);
    }
  });

  it("removes legacy MessageRow selectors from base.css and shell-refresh.css", () => {
    const baseCss = readFileSync(baseCssPath, "utf8");
    const shellRefreshCss = readFileSync(shellRefreshCssPath, "utf8");

    const removedBaseSelectors = [
      ".message-row p {",
      ".message-row p + p {",
      ".message-row strong {",
      ".message-row span {",
      ".reaction-row {",
      ".reaction-controls {",
      ".reaction-list {",
      ".reaction-chip {",
      ".reaction-chip.reacted {",
      ".message-actions {",
      ".message-row .icon-mask {",
      ".message-attachments {",
      ".message-attachment-card {",
      ".message-attachment-download {",
      ".message-attachment-meta {",
      ".message-attachment-failed {",
      ".message-attachment-retry {",
      ".message-edit input {",
    ];

    const removedShellRefreshSelectors = [
      ".message-avatar {",
      ".message-avatar-button {",
      ".message-avatar-fallback {",
      ".message-avatar-image {",
      ".message-main {",
      ".message-meta {",
      ".message-tokenized {",
      ".message-row:hover .message-hover-actions,",
      ".message-hover-actions .icon-button {",
      ".message-hover-actions .icon-button.danger {",
      ".message-row + .message-row {",
      ".reaction-row button {",
    ];

    for (const selector of removedBaseSelectors) {
      expect(baseCss).not.toContain(selector);
    }

    for (const selector of removedShellRefreshSelectors) {
      expect(shellRefreshCss).not.toContain(selector);
    }
  });

  it("removes legacy ServerRail selectors from base.css and shell-refresh.css", () => {
    const baseCss = readFileSync(baseCssPath, "utf8");
    const shellRefreshCss = readFileSync(shellRefreshCssPath, "utf8");

    const removedBaseSelectors = [
      ".server-rail .rail-label {",
      ".server-rail button {",
      ".server-rail button:hover {",
      ".server-rail button.active {",
    ];

    const removedShellRefreshSelectors = [
      ".server-list {",
      ".server-rail-footer {",
      ".server-action {",
      ".server-rail .rail-label {",
      ".server-rail button {",
      ".server-rail button:hover {",
      ".server-rail button.active {",
    ];

    for (const selector of removedBaseSelectors) {
      expect(baseCss).not.toContain(selector);
    }

    for (const selector of removedShellRefreshSelectors) {
      expect(shellRefreshCss).not.toContain(selector);
    }
  });

  it("uses shared tokens for the voice disconnect danger button style", () => {
    const channelRail = readFileSync(channelRailPath, "utf8");
    const match = channelRail.match(
      /voice-dock-disconnect-button[\s\S]*?style="([^"]+)"/,
    );
    expect(match).not.toBeNull();
    const styleValue = match![1];
    expect(styleValue).toContain("background: var(--danger-panel)");
    expect(styleValue).toContain("border-color: var(--danger-panel-strong)");
    expect(styleValue).toContain("color: var(--danger-ink)");
    expect(styleValue).not.toMatch(/#[0-9a-f]{3,8}/i);
  });

  it("forbids raw color literals in migrated TSX surfaces", () => {
    for (const migratedTsxPath of migratedTsxPaths) {
      const componentSource = readFileSync(migratedTsxPath, "utf8");
      expect(componentSource).not.toMatch(rawColorLiteralPattern);
    }
  });
});
