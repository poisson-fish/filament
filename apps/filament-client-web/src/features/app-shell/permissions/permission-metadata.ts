import type { PermissionName } from "../../../domain/chat";

export interface PermissionMatrixEntry {
  permission: PermissionName;
  label: string;
  summary: string;
  category: "workspace" | "moderation" | "voice" | "compatibility";
}

export interface PermissionCategory {
  key: PermissionMatrixEntry["category"];
  title: string;
}

export const PERMISSION_MATRIX: readonly PermissionMatrixEntry[] = [
  {
    permission: "create_message",
    label: "Create Messages",
    summary: "Send messages and participate in channels.",
    category: "workspace",
  },
  {
    permission: "delete_message",
    label: "Delete Messages",
    summary: "Delete or edit messages authored by other members.",
    category: "moderation",
  },
  {
    permission: "manage_channel_overrides",
    label: "Manage Overrides",
    summary: "Edit channel role override rules.",
    category: "workspace",
  },
  {
    permission: "ban_member",
    label: "Ban Members",
    summary: "Kick and ban users at workspace scope.",
    category: "moderation",
  },
  {
    permission: "manage_member_roles",
    label: "Manage Member Roles",
    summary: "Assign and unassign workspace roles on members.",
    category: "workspace",
  },
  {
    permission: "manage_workspace_roles",
    label: "Manage Workspace Roles",
    summary: "Create, update, delete, and reorder workspace roles.",
    category: "workspace",
  },
  {
    permission: "view_audit_log",
    label: "View Audit Log",
    summary: "Read redacted workspace audit history.",
    category: "moderation",
  },
  {
    permission: "manage_ip_bans",
    label: "Manage IP Bans",
    summary: "Apply and remove user-derived guild IP bans.",
    category: "moderation",
  },
  {
    permission: "publish_video",
    label: "Publish Camera",
    summary: "Publish camera tracks in voice channels.",
    category: "voice",
  },
  {
    permission: "publish_screen_share",
    label: "Publish Screen",
    summary: "Publish screen-share tracks in voice channels.",
    category: "voice",
  },
  {
    permission: "subscribe_streams",
    label: "Subscribe Streams",
    summary: "Receive remote media streams in voice channels.",
    category: "voice",
  },
  {
    permission: "manage_roles",
    label: "Legacy Manage Roles",
    summary: "Compatibility grant for pre-phase-7 moderation paths.",
    category: "compatibility",
  },
];

export const PERMISSION_CATEGORIES: readonly PermissionCategory[] = [
  { key: "workspace", title: "Workspace Access" },
  { key: "moderation", title: "Moderation" },
  { key: "voice", title: "Voice & Media" },
  { key: "compatibility", title: "Compatibility" },
];
