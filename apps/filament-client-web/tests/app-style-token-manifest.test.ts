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
const styleGovernancePath = resolve(webRootDir, "src/styles/STYLE_GOVERNANCE.md");
const channelRailPath = resolve(
  webRootDir,
  "src/features/app-shell/components/ChannelRail.tsx",
);
const serverRailPath = resolve(
  webRootDir,
  "src/features/app-shell/components/ServerRail.tsx",
);
const memberRailPath = resolve(
  webRootDir,
  "src/features/app-shell/components/MemberRail.tsx",
);
const chatHeaderPath = resolve(
  webRootDir,
  "src/features/app-shell/components/ChatHeader.tsx",
);
const userProfileOverlayPath = resolve(
  webRootDir,
  "src/features/app-shell/components/overlays/UserProfileOverlay.tsx",
);
const settingsPanelPath = resolve(
  webRootDir,
  "src/features/app-shell/components/panels/SettingsPanel.tsx",
);
const workspaceSettingsPanelPath = resolve(
  webRootDir,
  "src/features/app-shell/components/panels/WorkspaceSettingsPanel.tsx",
);
const panelHostPath = resolve(
  webRootDir,
  "src/features/app-shell/components/panels/PanelHost.tsx",
);
const utilityPanelPath = resolve(
  webRootDir,
  "src/features/app-shell/components/panels/UtilityPanel.tsx",
);
const chatColumnPath = resolve(
  webRootDir,
  "src/features/app-shell/components/layout/ChatColumn.tsx",
);
const publicDirectoryPanelPath = resolve(
  webRootDir,
  "src/features/app-shell/components/panels/PublicDirectoryPanel.tsx",
);
const friendshipsPanelPath = resolve(
  webRootDir,
  "src/features/app-shell/components/panels/FriendshipsPanel.tsx",
);
const attachmentsPanelPath = resolve(
  webRootDir,
  "src/features/app-shell/components/panels/AttachmentsPanel.tsx",
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
const loginPagePath = resolve(webRootDir, "src/pages/LoginPage.tsx");
const migratedTsxPaths = [
  channelRailPath,
  serverRailPath,
  memberRailPath,
  chatHeaderPath,
  userProfileOverlayPath,
  settingsPanelPath,
  workspaceSettingsPanelPath,
  panelHostPath,
  utilityPanelPath,
  chatColumnPath,
  publicDirectoryPanelPath,
  friendshipsPanelPath,
  attachmentsPanelPath,
  messageComposerPath,
  messageRowPath,
  reactionPickerPortalPath,
  loginPagePath,
];

const rawColorLiteralPattern = /#[0-9a-f]{3,8}\b|\b(?:rgb|rgba|hsl|hsla)\(/i;

function parseImportPaths(cssSource: string): string[] {
  return [...cssSource.matchAll(/@import\s+"([^"]+)";/g)].map((match) => match[1]);
}

describe("app style token manifest", () => {
  it("documents UnoCSS style governance requirements", () => {
    const styleGovernance = readFileSync(styleGovernancePath, "utf8");
    expect(styleGovernance).toContain("## When To Use Inline Utilities vs Shortcuts");
    expect(styleGovernance).toContain("## Variant And State Conventions");
    expect(styleGovernance).toContain("## Token-Only Color Policy");
    expect(styleGovernance).toContain("fx-*");
  });

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

  it("removes legacy ChannelRail selectors from base.css and shell-refresh.css", () => {
    const baseCss = readFileSync(baseCssPath, "utf8");
    const shellRefreshCss = readFileSync(shellRefreshCssPath, "utf8");

    const removedBaseSelectors = [
      ".channel-rail nav {",
      ".channel-rail nav button {",
      ".channel-rail nav button:hover {",
      ".channel-rail nav button.active {",
    ];

    const removedShellRefreshSelectors = [
      ".workspace-menu-trigger {",
      ".workspace-menu-item {",
      ".workspace-menu-divider {",
      ".channel-nav {",
      ".channel-nav .channel-row {",
      ".voice-connected-dock {",
      ".voice-dock-icon-button {",
      ".channel-rail-account-bar {",
      ".channel-rail-account-action {",
      ".voice-channel-stream-hints {",
      ".voice-participant-muted-badge {",
      ".voice-participant-deafened-badge {",
      ".voice-participant-media-badge {",
    ];

    for (const selector of removedBaseSelectors) {
      expect(baseCss).not.toContain(selector);
    }

    for (const selector of removedShellRefreshSelectors) {
      expect(shellRefreshCss).not.toContain(selector);
    }
  });

  it("removes legacy MemberRail selectors from base.css and shell-refresh.css", () => {
    const baseCss = readFileSync(baseCssPath, "utf8");
    const shellRefreshCss = readFileSync(shellRefreshCssPath, "utf8");

    const removedBaseSelectors = [
      ".member-rail h4",
      ".profile-card {",
      ".profile-card p {",
      ".profile-card p + p {",
      ".profile-card .label {",
      ".ops-launch-grid {",
      ".ops-launch-grid button {",
    ];

    const removedShellRefreshSelectors = [
      ".member-rail h4",
      ".profile-card {",
      ".ops-launch-grid button {",
      ".ops-launch-grid {",
    ];

    for (const selector of removedBaseSelectors) {
      expect(baseCss).not.toContain(selector);
    }

    for (const selector of removedShellRefreshSelectors) {
      expect(shellRefreshCss).not.toContain(selector);
    }
  });

  it("removes legacy ChatHeader selectors from base.css and shell-refresh.css", () => {
    const baseCss = readFileSync(baseCssPath, "utf8");
    const shellRefreshCss = readFileSync(shellRefreshCssPath, "utf8");

    const removedBaseSelectors = [
      ".chat-header h3 {",
      ".chat-header {",
      ".chat-header p {",
      ".header-actions {",
      ".header-actions button,",
      ".logout {",
      ".gateway-badge {",
      ".gateway-badge.online {",
      ".voice-badge {",
      ".voice-badge.connecting,",
      ".voice-badge.connected {",
      ".voice-badge.error {",
    ];

    const removedShellRefreshSelectors = [
      ".chat-header h3 {",
      ".chat-header {",
      ".chat-header p {",
      ".header-actions {",
      ".header-actions button,",
      ".logout {",
      ".gateway-badge {",
      ".gateway-badge.online {",
      ".voice-badge {",
      ".voice-badge.connecting,",
      ".voice-badge.connected {",
      ".voice-badge.error {",
      ".header-actions .header-icon-button {",
      ".header-actions .header-icon-button .icon-mask {",
      ".header-actions .header-icon-button:hover:not(:disabled) {",
      ".header-actions .header-icon-button:disabled {",
      ".header-actions .header-icon-button.logout {",
      ".header-actions .header-icon-button.logout:hover:not(:disabled) {",
    ];

    for (const selector of removedBaseSelectors) {
      expect(baseCss).not.toContain(selector);
    }

    for (const selector of removedShellRefreshSelectors) {
      expect(shellRefreshCss).not.toContain(selector);
    }
  });

  it("removes legacy UserProfileOverlay selectors from shell-refresh.css", () => {
    const shellRefreshCss = readFileSync(shellRefreshCssPath, "utf8");

    const removedShellRefreshSelectors = [
      ".profile-view-panel {",
      ".profile-view-body {",
      ".profile-view-header {",
      ".profile-view-avatar {",
      ".profile-view-avatar-fallback {",
      ".profile-view-avatar-image {",
      ".profile-view-name {",
      ".profile-view-markdown {",
      ".profile-view-markdown p",
      ".profile-view-markdown p + p",
      ".profile-view-markdown ul",
      ".profile-view-markdown ol",
    ];

    for (const selector of removedShellRefreshSelectors) {
      expect(shellRefreshCss).not.toContain(selector);
    }
  });

  it("removes legacy SettingsPanel selectors from shell-refresh.css", () => {
    const shellRefreshCss = readFileSync(shellRefreshCssPath, "utf8");

    const removedShellRefreshSelectors = [
      ".settings-panel-layout {",
      ".settings-panel-rail {",
      ".settings-panel-content {",
      ".settings-category-list {",
      ".settings-category-button {",
      ".settings-category-button-active {",
      ".settings-category-name {",
      ".settings-category-summary {",
      ".settings-submenu-layout {",
      ".settings-submenu-rail {",
      ".settings-submenu-content {",
      ".settings-submenu-list {",
      ".settings-submenu-button {",
      ".settings-submenu-button-active {",
      ".settings-profile-actions {",
      ".settings-profile-preview {",
      ".settings-profile-preview-head {",
      ".settings-avatar-shell {",
      ".settings-avatar-fallback {",
      ".settings-avatar-image {",
      ".settings-profile-name {",
      ".settings-profile-markdown {",
      ".settings-profile-markdown p {",
      ".settings-profile-markdown p + p {",
    ];

    for (const selector of removedShellRefreshSelectors) {
      expect(shellRefreshCss).not.toContain(selector);
    }
  });

  it("removes legacy auth shell selectors from base.css", () => {
    const baseCss = readFileSync(baseCssPath, "utf8");

    const removedBaseSelectors = [
      ".auth-layout {",
      ".auth-panel {",
      ".auth-header h1 {",
      ".eyebrow {",
      ".auth-mode-switch {",
      ".auth-mode-switch button {",
      ".auth-mode-switch button.active {",
      ".auth-mode-switch button:hover {",
      ".auth-form {",
      ".auth-form label {",
      ".auth-form input {",
      ".auth-form input:focus-visible {",
      ".captcha-block {",
      ".captcha-block .status {",
      ".auth-form button[type=\"submit\"] {",
      ".auth-form button[type=\"submit\"]:disabled {",
    ];

    for (const selector of removedBaseSelectors) {
      expect(baseCss).not.toContain(selector);
    }
  });

  it("removes legacy panel host selectors from shell-refresh.css", () => {
    const shellRefreshCss = readFileSync(shellRefreshCssPath, "utf8");

    const removedShellRefreshSelectors = [
      ".panel-backdrop {",
      ".panel-window {",
      ".panel-window-medium {",
      ".panel-window-compact {",
      ".panel-window-header {",
      ".panel-window-header h4 {",
      ".panel-window-header button {",
      ".panel-window-body {",
    ];

    for (const selector of removedShellRefreshSelectors) {
      expect(shellRefreshCss).not.toContain(selector);
    }
  });

  it("removes legacy public directory selectors from base.css", () => {
    const baseCss = readFileSync(baseCssPath, "utf8");

    const removedBaseSelectors = [
      ".public-directory {",
      ".public-directory ul {",
      ".public-directory li {",
      ".public-directory-row-main {",
      ".public-directory-row-actions {",
      ".directory-status-chip {",
      ".directory-status-chip.joined {",
      ".directory-status-chip.banned,",
      ".directory-status-chip.pending {",
      ".public-directory-row-error {",
      ".unread-count {",
    ];

    for (const selector of removedBaseSelectors) {
      expect(baseCss).not.toContain(selector);
    }
  });

  it("removes dead load-older, workspace-create, panel-note, stacked-meta, and mono selectors from base.css", () => {
    const baseCss = readFileSync(baseCssPath, "utf8");

    const removedBaseSelectors = [
      ".load-older {",
      ".workspace-create-panel {",
      ".workspace-create-panel h4 {",
      ".panel-note {",
      ".stacked-meta {",
      ".mono {",
    ];

    for (const selector of removedBaseSelectors) {
      expect(baseCss).not.toContain(selector);
    }
  });

  it("removes dead voice roster, voice video grid, and reaction add trigger selectors from base.css", () => {
    const baseCss = readFileSync(baseCssPath, "utf8");

    const removedBaseSelectors = [
      ".voice-roster {",
      ".voice-roster-title {",
      ".voice-roster-empty {",
      ".voice-roster ul {",
      ".voice-roster li {",
      ".voice-roster li span:last-child {",
      ".voice-roster-local {",
      ".voice-roster-speaking {",
      ".voice-roster-name {",
      ".voice-roster-name-speaking {",
      ".voice-stream-hints {",
      ".voice-stream-hints p {",
      ".voice-video-grid {",
      ".voice-video-grid-title {",
      ".voice-video-grid-empty {",
      ".voice-video-grid-tiles {",
      ".voice-video-tile {",
      ".voice-video-tile-local {",
      ".voice-video-tile-screen {",
      ".voice-video-tile-identity {",
      ".voice-video-tile-meta {",
      ".voice-video-grid-overflow {",
      ".reaction-add-trigger {",
      ".reaction-add-trigger:hover {",
    ];

    for (const selector of removedBaseSelectors) {
      expect(baseCss).not.toContain(selector);
    }
  });

  it("removes legacy group-label selectors from base.css and shell-refresh.css", () => {
    const baseCss = readFileSync(baseCssPath, "utf8");
    const shellRefreshCss = readFileSync(shellRefreshCssPath, "utf8");

    expect(baseCss).not.toContain(".group-label {");
    expect(baseCss).not.toContain(".ops-overlay-header .group-label {");
    expect(shellRefreshCss).not.toContain(".group-label {");
  });

  it("uses shared tokens for the voice disconnect danger button utility classes", () => {
    const channelRail = readFileSync(channelRailPath, "utf8");
    expect(channelRail).toContain("bg-danger-panel");
    expect(channelRail).toContain("border-danger-panel-strong");
    expect(channelRail).toContain("text-danger-ink");
    expect(channelRail).not.toContain("voice-dock-disconnect-button");
  });

  it("forbids raw color literals in migrated TSX surfaces", () => {
    for (const migratedTsxPath of migratedTsxPaths) {
      const componentSource = readFileSync(migratedTsxPath, "utf8");
      expect(componentSource).not.toMatch(rawColorLiteralPattern);
    }
  });
});
