import type {
  ChannelPermissionSnapshot,
  GuildRoleRecord,
  PermissionName,
  RoleName,
  UserId,
  WorkspaceRoleId,
} from "../../../domain/chat";
import type {
  WorkspaceChannelOverrideRecord,
} from "../state/workspace-state";

const MAX_PERMISSIONS_PER_ROLE = 64;
const MAX_TRACKED_ROLE_ASSIGNMENTS_PER_USER = 64;
const MAX_TRACKED_LEGACY_CHANNEL_OVERRIDES = 16;

type PermissionBits = number;

export interface ChannelPermissionOverwrite {
  allow: PermissionBits;
  deny: PermissionBits;
}

interface LegacyRoleBuckets {
  everyoneRoleIds: Set<WorkspaceRoleId>;
  ownerRoleIds: Set<WorkspaceRoleId>;
  moderatorRoleIds: Set<WorkspaceRoleId>;
  memberRoleIds: Set<WorkspaceRoleId>;
}

export interface ResolveEffectiveChannelPermissionsInput {
  channelPermissionsSnapshot: ChannelPermissionSnapshot | null;
  guildRoles: ReadonlyArray<GuildRoleRecord>;
  assignedRoleIds: ReadonlyArray<WorkspaceRoleId>;
  channelOverrides: ReadonlyArray<WorkspaceChannelOverrideRecord>;
}

export interface ResolveEffectiveLegacyRolePermissionsInput {
  role: RoleName;
  guildRoles: ReadonlyArray<GuildRoleRecord>;
  channelOverrides: ReadonlyArray<WorkspaceChannelOverrideRecord>;
}

export const KNOWN_PERMISSIONS: readonly PermissionName[] = [
  "manage_roles",
  "manage_member_roles",
  "manage_workspace_roles",
  "manage_channel_overrides",
  "delete_message",
  "ban_member",
  "view_audit_log",
  "manage_ip_bans",
  "create_message",
  "publish_video",
  "publish_screen_share",
  "subscribe_streams",
];

const PERMISSION_BITS: Record<PermissionName, PermissionBits> = {
  manage_roles: 1 << 0,
  manage_member_roles: 1 << 1,
  manage_workspace_roles: 1 << 2,
  manage_channel_overrides: 1 << 3,
  delete_message: 1 << 4,
  ban_member: 1 << 5,
  view_audit_log: 1 << 6,
  manage_ip_bans: 1 << 7,
  create_message: 1 << 8,
  publish_video: 1 << 9,
  publish_screen_share: 1 << 10,
  subscribe_streams: 1 << 11,
};

const KNOWN_PERMISSION_MASK = KNOWN_PERMISSIONS.reduce<PermissionBits>(
  (bits, permission) => bits | PERMISSION_BITS[permission],
  0,
);

export function hasPermission(
  permissionBits: PermissionBits,
  permission: PermissionName,
): boolean {
  return (permissionBits & PERMISSION_BITS[permission]) !== 0;
}

export function permissionBitsFromList(
  permissions: ReadonlyArray<PermissionName>,
): PermissionBits {
  let bits = 0;
  const bounded = permissions.slice(0, MAX_PERMISSIONS_PER_ROLE);
  for (const permission of bounded) {
    bits |= PERMISSION_BITS[permission];
  }
  return bits & KNOWN_PERMISSION_MASK;
}

export function permissionListFromBits(permissionBits: PermissionBits): PermissionName[] {
  const maskedBits = permissionBits & KNOWN_PERMISSION_MASK;
  return KNOWN_PERMISSIONS.filter(
    (permission) => (maskedBits & PERMISSION_BITS[permission]) !== 0,
  );
}

export function computeBasePermissions(
  rolePermissionSets: ReadonlyArray<PermissionBits>,
): PermissionBits {
  let bits = 0;
  for (const permissionSet of rolePermissionSets) {
    bits |= permissionSet;
  }
  return bits & KNOWN_PERMISSION_MASK;
}

export function applyChannelOverrides(
  isWorkspaceOwner: boolean,
  basePermissions: PermissionBits,
  everyoneOverride: ChannelPermissionOverwrite | null,
  roleOverrides: ReadonlyArray<ChannelPermissionOverwrite>,
  memberOverride: ChannelPermissionOverwrite | null,
): PermissionBits {
  if (isWorkspaceOwner) {
    return KNOWN_PERMISSION_MASK;
  }

  let current = basePermissions & KNOWN_PERMISSION_MASK;

  if (everyoneOverride) {
    const normalized = normalizeOverwriteLayer(everyoneOverride);
    current &= ~normalized.deny;
    current |= normalized.allow;
  }

  let roleAllow = 0;
  let roleDeny = 0;
  for (const roleOverride of roleOverrides) {
    const normalized = normalizeOverwriteLayer(roleOverride);
    roleAllow |= normalized.allow;
    roleDeny |= normalized.deny;
  }
  current &= ~roleDeny;
  current |= roleAllow;

  if (memberOverride) {
    const normalized = normalizeOverwriteLayer(memberOverride);
    current &= ~normalized.deny;
    current |= normalized.allow;
  }

  return current & KNOWN_PERMISSION_MASK;
}

export function resolveEffectiveChannelPermissions(
  input: ResolveEffectiveChannelPermissionsInput,
): PermissionBits {
  const roleById = new Map<WorkspaceRoleId, GuildRoleRecord>();
  for (const role of input.guildRoles) {
    roleById.set(role.roleId, role);
    if (roleById.size >= 64) {
      break;
    }
  }

  if (roleById.size === 0) {
    return permissionBitsFromList(input.channelPermissionsSnapshot?.permissions ?? []);
  }

  const legacyRoleBuckets = buildLegacyRoleBuckets(input.guildRoles);
  const effectiveRoleIds = resolveEffectiveRoleIds(
    input.assignedRoleIds,
    input.channelPermissionsSnapshot?.role ?? null,
    legacyRoleBuckets,
  );

  const rolePermissionSets: PermissionBits[] = [];
  for (const roleId of effectiveRoleIds) {
    const role = roleById.get(roleId);
    if (!role) {
      continue;
    }
    rolePermissionSets.push(permissionBitsFromList(role.permissions));
  }

  let guildPermissionBits = computeBasePermissions(rolePermissionSets);
  const isWorkspaceOwner = intersects(effectiveRoleIds, legacyRoleBuckets.ownerRoleIds);
  if (isWorkspaceOwner) {
    guildPermissionBits = KNOWN_PERMISSION_MASK;
  }

  const scopedChannelOverrides = input.channelOverrides.slice(
    0,
    MAX_TRACKED_LEGACY_CHANNEL_OVERRIDES,
  );
  const roleOverrides: ChannelPermissionOverwrite[] = [];
  for (const override of scopedChannelOverrides) {
    if (override.targetKind !== "legacy_role") {
      continue;
    }
    if (!legacyRoleAppliesToUser(override.role, legacyRoleBuckets, effectiveRoleIds)) {
      continue;
    }
    roleOverrides.push({
      allow: permissionBitsFromList(override.allow),
      deny: permissionBitsFromList(override.deny),
    });
  }

  const localBits = applyChannelOverrides(
    isWorkspaceOwner,
    guildPermissionBits,
    null,
    roleOverrides,
    null,
  );

  if (localBits === 0 && input.channelPermissionsSnapshot) {
    return permissionBitsFromList(input.channelPermissionsSnapshot.permissions);
  }

  return localBits;
}

export function resolveEffectiveLegacyRolePermissions(
  input: ResolveEffectiveLegacyRolePermissionsInput,
): PermissionBits {
  const roleById = new Map<WorkspaceRoleId, GuildRoleRecord>();
  for (const role of input.guildRoles) {
    roleById.set(role.roleId, role);
    if (roleById.size >= 64) {
      break;
    }
  }

  if (roleById.size === 0) {
    return 0;
  }

  const legacyRoleBuckets = buildLegacyRoleBuckets(input.guildRoles);
  const effectiveRoleIds = new Set<WorkspaceRoleId>();
  for (const roleId of legacyRoleBuckets.everyoneRoleIds) {
    effectiveRoleIds.add(roleId);
  }
  for (const roleId of roleIdsForLegacyRole(input.role, legacyRoleBuckets)) {
    effectiveRoleIds.add(roleId);
  }

  const rolePermissionSets: PermissionBits[] = [];
  for (const roleId of effectiveRoleIds) {
    const role = roleById.get(roleId);
    if (!role) {
      continue;
    }
    rolePermissionSets.push(permissionBitsFromList(role.permissions));
  }

  let guildPermissionBits = computeBasePermissions(rolePermissionSets);
  const isWorkspaceOwner = intersects(effectiveRoleIds, legacyRoleBuckets.ownerRoleIds);
  if (isWorkspaceOwner) {
    guildPermissionBits = KNOWN_PERMISSION_MASK;
  }

  const roleOverrides: ChannelPermissionOverwrite[] = [];
  const scopedChannelOverrides = input.channelOverrides.slice(
    0,
    MAX_TRACKED_LEGACY_CHANNEL_OVERRIDES,
  );
  for (const override of scopedChannelOverrides) {
    if (override.targetKind !== "legacy_role") {
      continue;
    }
    if (!legacyRoleAppliesToUser(override.role, legacyRoleBuckets, effectiveRoleIds)) {
      continue;
    }
    roleOverrides.push({
      allow: permissionBitsFromList(override.allow),
      deny: permissionBitsFromList(override.deny),
    });
  }

  return applyChannelOverrides(
    isWorkspaceOwner,
    guildPermissionBits,
    null,
    roleOverrides,
    null,
  );
}

export function resolveAssignedRoleIdsForUser(
  currentUserId: UserId | null,
  userRoleAssignments: Record<string, WorkspaceRoleId[]> | undefined,
): WorkspaceRoleId[] {
  if (!currentUserId || !userRoleAssignments) {
    return [];
  }
  const assigned = userRoleAssignments[currentUserId];
  if (!assigned) {
    return [];
  }
  if (Array.isArray(assigned)) {
    return dedupeRoleIds(assigned.slice(0, MAX_TRACKED_ROLE_ASSIGNMENTS_PER_USER));
  }
  return [];
}

function normalizeOverwriteLayer(
  overwrite: ChannelPermissionOverwrite,
): ChannelPermissionOverwrite {
  const deny = overwrite.deny & KNOWN_PERMISSION_MASK;
  const allow = (overwrite.allow & KNOWN_PERMISSION_MASK) & ~deny;
  return { allow, deny };
}

function dedupeRoleIds(roleIds: ReadonlyArray<WorkspaceRoleId>): WorkspaceRoleId[] {
  const deduped: WorkspaceRoleId[] = [];
  const seen = new Set<WorkspaceRoleId>();
  for (const roleId of roleIds) {
    if (seen.has(roleId)) {
      continue;
    }
    seen.add(roleId);
    deduped.push(roleId);
  }
  return deduped;
}

function resolveEffectiveRoleIds(
  assignedRoleIds: ReadonlyArray<WorkspaceRoleId>,
  snapshotRole: RoleName | null,
  buckets: LegacyRoleBuckets,
): Set<WorkspaceRoleId> {
  const dedupedAssignedRoleIds = dedupeRoleIds(
    assignedRoleIds.slice(0, MAX_TRACKED_ROLE_ASSIGNMENTS_PER_USER),
  );
  const effective = new Set<WorkspaceRoleId>(dedupedAssignedRoleIds);
  for (const roleId of buckets.everyoneRoleIds) {
    effective.add(roleId);
  }

  if (dedupedAssignedRoleIds.length === 0 && snapshotRole) {
    for (const roleId of roleIdsForLegacyRole(snapshotRole, buckets)) {
      effective.add(roleId);
    }
  }

  return effective;
}

function buildLegacyRoleBuckets(
  roles: ReadonlyArray<GuildRoleRecord>,
): LegacyRoleBuckets {
  const buckets: LegacyRoleBuckets = {
    everyoneRoleIds: new Set<WorkspaceRoleId>(),
    ownerRoleIds: new Set<WorkspaceRoleId>(),
    moderatorRoleIds: new Set<WorkspaceRoleId>(),
    memberRoleIds: new Set<WorkspaceRoleId>(),
  };
  for (const role of roles) {
    const normalizedName = role.name.trim().toLowerCase();
    const canonicalName = normalizedName.replace(/[\s-]+/g, "_");
    if (
      normalizedName === "@everyone" ||
      (role.isSystem && role.position === 0)
    ) {
      buckets.everyoneRoleIds.add(role.roleId);
      continue;
    }
    if (canonicalName === "workspace_owner" || canonicalName === "owner") {
      buckets.ownerRoleIds.add(role.roleId);
      continue;
    }
    if (canonicalName === "moderator") {
      buckets.moderatorRoleIds.add(role.roleId);
      continue;
    }
    if (canonicalName === "member") {
      buckets.memberRoleIds.add(role.roleId);
    }
  }
  return buckets;
}

function legacyRoleAppliesToUser(
  role: RoleName,
  buckets: LegacyRoleBuckets,
  roleIds: Set<WorkspaceRoleId>,
): boolean {
  const candidates = roleIdsForLegacyRole(role, buckets);
  return intersects(roleIds, candidates);
}

function roleIdsForLegacyRole(
  role: RoleName,
  buckets: LegacyRoleBuckets,
): Set<WorkspaceRoleId> {
  switch (role) {
    case "owner":
      return buckets.ownerRoleIds;
    case "moderator":
      return buckets.moderatorRoleIds;
    case "member":
      return buckets.memberRoleIds;
    default:
      return new Set<WorkspaceRoleId>();
  }
}

function intersects(
  left: Set<WorkspaceRoleId>,
  right: Set<WorkspaceRoleId>,
): boolean {
  for (const value of left) {
    if (right.has(value)) {
      return true;
    }
  }
  return false;
}
