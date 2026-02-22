# Comprehensive Plan: Hierarchical Permissions System

This document outlines the detailed plan to fully integrate a robust, Discord-style hierarchical permissions system into Filament.

## Current State Analysis
Currently, Filament utilizes a hybrid system where database schemas (like `v8_permission_legacy_schema.rs`) support custom workspace roles (`guild_roles`, `guild_role_members`, and `channel_permission_overrides`), while the `filament_core` domain logic mostly centers around a hard-coded generic `Role` enum (`Owner`, `Moderator`, `Member`) and statically defined `base_permissions()`. The frontend has the typescript definitions ready (`GuildRoleRecord`, `PermissionName[]`, etc.), and some gateway event scaffolding, but full integration is pending.

## Goal
Transition from the static `Role` enum logic to a fully dynamic permission evaluation model. The system will calculate effective permissions for a user within a workspace (and optionally within specific channels) based on their assigned custom roles and channel-specific overrides.

---

## 1. Domain Logic Overhaul (`filament-core/src/lib.rs`)

We need to update the core library to compute access dynamically rather than relying solely on `can_assign_role(actor: Role, ...)` and `base_permissions(role: Role)`.

### 1.1 Shift from Static Role Enum to PermissionSets
- **Deprecate `Role` enum for authorization**: Keep the `Role` enum purely for determining ownership (the `Owner` can never be locked out) but eliminate static `base_permissions`.
- **Introduce `compute_base_permissions`**: A function taking a collection of `PermissionSet`s (the masks of all roles assigned to the user) and returning the bitwise `OR` combined `PermissionSet`.
- **System Roles (Sane Defaults)**: Use the `guild_roles.is_system` and `guild_roles.system_key` columns to identify the initial default roles that mimic the current static enum, preventing breakages for legacy workspaces. 

### 1.2 Channel Overrides Priority Model (Discord-Like)
Currently `apply_channel_overwrite` takes a single overwrite. We need to implement the standard hierarchical evaluation for channel overrides:
1. **Base Permissions**: Start with the combined `PermissionSet` from all workspace roles assigned to the user.
2. **Global Admins**: If the base permissions contain a master permission (e.g., `Administrator` if added, or if the user is the explicit Workspace Owner), instantly return full permissions.
3. **Apply Base Role Overrides**: Apply the `@everyone` (or system base role) channel override (Allow and Deny).
4. **Apply Other Role Overrides**: Collect all role-specific overrides for the user's roles. 
    - Deny rules are applied together.
    - Allow rules are applied together (Allow overrides Deny from other roles at this same level).
5. **Apply Member Overrides**: Apply the user-specific channel override (Targeted Deny, then Targeted Allow). This has the highest priority.

### 1.3 Role Hierarchy (Positioning)
- Ensure actions like `ManageMemberRoles`, `ManageWorkspaceRoles`, and moderating tasks (kick/ban) respect the `position` integer of the `guild_roles`.
- **Rule**: A user can only assign/modify/remove a role if its `position` is *lower* than the highest `position` role the actor holds.
- **Rule**: A user can only kick/ban a member whose highest role `position` is strictly lower than the actor's highest role `position`.

---

## 2. Default Roles and Permissions (Sane Defaults)

When a new workspace is created, the system should generate standard roles to provide immediate functionality. These defaults can be modified by the workspace owner later.

| Role Name | System Key | Position | Characteristics / Default Permissions |
| :--- | :--- | :---: | :--- |
| **Owner** | `owner` | 999 | Implicitly has all permissions. Cannot be deleted. |
| **Moderator** | `moderator` | 100 | `ManageMemberRoles`, `DeleteMessage`, `BanMember`, `ViewAuditLog`, `ManageIpBans`, etc. |
| **@everyone** | `everyone` | 0 | The base foundational role assigned to every member. `CreateMessage`, `SubscribeStreams`. Cannot be deleted. |

*Note: In the database, the workspace creator should be directly assigned the `owner` role, and any future users automatically fall under the evaluation of the `everyone` role.*

---

## 3. Backend Implementation Steps (`filament-server`)

### 3.1 Permission Application
- Replace checks like `has_permission(Role::Owner, Permission::CreateMessage)` across handlers and WebSockets.
- Introduce a utility in the request context or permission service like `check_workspace_permission(user_id, guild_id, permission)` and `check_channel_permission(user_id, channel_id, permission)`.
- These utilities should query `guild_roles` joined with `guild_role_members` to calculate the `allow_mask`.

### 3.2 Channel Overrides Refinement
- Update API endpoints to modify `channel_permission_overrides` specifying either `Role` or `Member` via `target_kind`.
- On any role/override changes, ensure LiveKit tokens and Gateway states are re-evaluated and clients are notified.

### 3.3 Gateway Event Integration
Ensure the following events are broadcast and fully wired into the client state:
- `WorkspaceRoleCreate` / `WorkspaceRoleUpdate` / `WorkspaceRoleDelete`
- `WorkspaceRoleAssignmentAdd` / `WorkspaceRoleAssignmentRemove`
- `WorkspaceChannelOverrideUpdate` / `WorkspaceChannelOverrideDelete`

---

## 4. Frontend Implementation Steps (`filament-client-web`)

### 4.1 State Management Updates
- Ensure the Pinia/Solid state (`workspace-state.ts`, `role-management-controller.ts`) tracks:
  - `roles`: `GuildRoleRecord[]` (ordered by position).
  - `userRoles`: Mapping of `UserId` to `WorkspaceRoleId[]`.
  - `channelOverrides`: Mapping of `ChannelId` to overrides.

### 4.2 Comprehensive UI Design for Permissions Management

Implementing Discord-like permissions requires a suite of complex user interfaces capable of managing hierarchical and channel-specific rules intuitively.

#### 4.2.1 Workspace Settings: Roles Panel
The central hub for managing workspace-wide roles.
- **Role List & Ordering**: 
  - A vertical sidebar displaying all roles in descending order of hierarchy/position.
  - The `@everyone` role is pinned at the bottom and cannot be reordered or deleted.
  - Integration of a drag-and-drop library (e.g., `@dnd-kit/core`) to allow users with `ManageWorkspaceRoles` to reorder the hierarchy.
- **Role Editor**: 
  - When clicking a role in the list, the main panel displays its properties.
  - **Header**: Input to edit the role's display name and an overarching "Delete Role" button.
  - **Permissions List**: A grouped list of toggles for every `PermissionName` (e.g., General Server Permissions, Text Permissions, Voice Permissions).
  - Each toggle directly maps to modifying the bitmask for the role.
  - Real-time save confirmation toast/indicator.

#### 4.2.2 Workspace Settings: Members Panel
For assigning the created roles to specific users.
- **Member Table**: Displays a list of all server members, paginated or virtualized.
- **Inline Role Assignment**: 
  - Clicking a member opens a popover / dropdown equipped with a searchable checklist of all workspace roles.
  - Selected roles appear as colored badges next to the user's name.
  - The UI must eagerly block the selection of roles ranked *higher* than the actor's highest role.

#### 4.2.3 Channel Settings Modal: Permissions Tab
The most complex UI piece, governing the layer-by-layer overrides for a specific channel.
- **Entity Selector Sidebar**: 
  - Lists the baseline `@everyone` role, alongside any Roles or Members that have *explicit overrides* set for this channel.
  - An "Add Role or Member" `(+)` button opens a searchable modal to select a target. Selected targets are then appended to the sidebar list.
- **Tri-State Permission Matrix**: 
  - When an entity (Role or Member) is selected from the sidebar, the main view exposes the permission overrides.
  - For every applicable `PermissionName`, present three distinct, mutually exclusive radio buttons or a segmented control: 
    1. **`/` (Inherit / Default)**: Removes the override (clears both Allow and Deny bits for this permission).
    2. **`✓` (Allow)**: Sets the Allow bit, ensuring the entity gets this permission regardless of base roles.
    3. **`X` (Deny)**: Sets the Deny bit, revoking the permission from this entity.
  - **Visual Feedback**: The UI should calculate and display the *effective* permission to the side or via tooltip, showing users what the resulting state is based on the hierarchy.

#### 4.2.4 UX Simplifications & Onboarding
To ensure this complex system remains accessible to the average user, we will incorporate several UX rails and onboarding features:
- **Role Templates**: When clicking "Create Role", offer standard presets alongside a "Custom Role" option. Presets could include:
  - *Cosmetic Role*: Inherits all permissions from `@everyone` but changes name color.
  - *Moderator*: Pre-selects common moderation capabilities (`DeleteMessage`, `BanMember`).
  - *Read-Only*: Explicitly blocks `CreateMessage` and `PublishVideo`.
- **In-App Explanations**: 
  - Subtext under each permission toggle explaining its real-world effect (e.g., under `ManageWorkspaceRoles`: "Allows members to create, edit, or delete roles lower than their highest role.").
  - Use clear, non-technical language.
- **"View As Role" Simulator**: 
  - Within the Workspace Settings, provide a "View Server As Role" toggle. This artificially clamps the client's current session permissions allowing admins to visually confirm what a regular `@everyone` or `@guest` user can see and do without needing a dummy account.
- **Dangerous Action Warnings**: 
  - Add explicit confirmation modals when modifying or saving permissions that could result in losing control (e.g., removing `ManageWorkspaceRoles` from themselves) or granting extreme escalation vectors (e.g., giving `Administrator`/full permissions to a widespread role).

### 4.3 Client-Side Permission Evaluation
- Implement the exact same override evaluation algorithm (Section 1.2) on the frontend to eagerly hide/show UI elements (like the "Delete Message" trash can icon or the "Server Settings" button).

---

## 5. Migration Strategy for Existing Workspaces

If workspaces exist using the legacy `guild_members.role SMALLINT` column:
1. Provide a migration script within `apply_permission_legacy_schema` or the backend startup.
2. For each workspace, create the default `owner`, `moderator`, and `everyone` roles in `guild_roles`.
3. For every member in `guild_members`, read their `role` INT. If they were an Owner/Moderator, insert a row in `guild_role_members` assigning them the corresponding newly created custom role.

---

## Summary of Next Actions
1. **Refactor `filament_core`**: Remove fixed masks; implement `apply_channel_overrides(member_roles: Vec<PermissionSet>, user_allow, user_deny, role_allows, role_denies)`.
2. **Backend Queries**: Create central optimized functions to fetch a user's `PermissionSet` for a workspace and channel.
3. **Frontend Polish**: Build the tri-state Channel Overrides UI and standard Role Management screen with drag-and-drop array reordering.

---

## 6. Implementation Phases & Status

This section is used to track the progress of the implementation across multiple sessions. When a phase is completed, mark the checkbox `[x]` and add any relevant notes.

### Phase 1: Domain & Database Foundation
- [x] Migrate ` filament_core` static `Role` logic to dynamic `PermissionSet` evaluation via `compute_base_permissions`.
- [x] Implement the Discord-like 5-step Channel Overrides Priority Model in `apply_channel_overwrite`.
- [x] Finalize role hierarchy rules (`position` comparisons for assigning/moderating).
- [x] Write the database migration script for existing workspaces (legacy role column to `guild_roles` tables).
- **Notes**: 
  - *Completed Phase 1. Added `compute_base_permissions` and `apply_channel_overrides` implementing the 5-step Discord priority logic. Updated `can_assign_role` and `can_moderate_member` to evaluate position rankings dynamically. Set up legacy schema data backfill loop to automatically populate standard `Owner`, `Moderator`, and `@everyone` roles as well as mapping current `guild_members.role` mapping in `guild_role_members`.*

### Phase 2: Backend Integration & API
- [x] Create central context/service utilities (`check_workspace_permission`, `check_channel_permission`).
- [ ] Update API routes handling creation, modification, and deletion of roles (`guild_roles`).
- [ ] Update API routes for channel permission overrides (`channel_permission_overrides`).
- [ ] Wire up LiveKit token updates when a user's permissions change.
- [ ] Wire Gateway Events (`WorkspaceRoleCreate`, `WorkspaceChannelOverrideUpdate`, etc.) to broadcast on changes.
- **Notes**: 
  - *Implemented `check_workspace_permission` and `check_channel_permission` in `domain.rs` to validate if a user has a specific permission via `guild_permission_snapshot` and `channel_permission_snapshot`. Ready to be used for replacing legacy `has_permission` checks.*

### Phase 3: Frontend Data & State Handling
- [ ] Update Pinia/Solid state models to track `roles` (ordered array), `userRoles`, and `channelOverrides`.
- [ ] Implement Gateway Event listeners to seamlessly patch the local state.
- [ ] Port the exact `apply_channel_overwrite` and `compute_base_permissions` logic to `filament-client-web` to selectively hide/show UI elements (buttons, inputs) based on effective permissions.
- **Notes**: 
  - *(Add implementation notes here)*

### Phase 4: Frontend UI - Role & Member Management
- [ ] Build the **Workspace Settings: Roles Panel** (Sidebar list, drag-and-drop hierarchy).
- [ ] Build the **Role Editor Panel** (Name editing, permission toggles list, save system).
- [ ] Integrate **Role Templates** (Cosmetic, Moderator, Read-Only) into creation flow.
- [ ] Build the **Workspace Settings: Members Panel** (Inline role assignment via dropdown badge UI).
- [ ] Add explicit warnings/confirmation modals for dangerous operations.
- **Notes**: 
  - *(Add implementation notes here)*

### Phase 5: Frontend UI - Channel Overrides
- [ ] Build the **Channel Settings: Permissions Tab** (Entity Selector Sidebar prioritizing `@everyone` + active overrides).
- [ ] Build the **Tri-State Permission Matrix** (`/`, `✓`, `X`) for the selected Role/Member.
- [ ] Implement visual indicators showing the calculated effective permission.
- **Notes**: 
  - *(Add implementation notes here)*

### Phase 6: Polish Phase
- [ ] Add in-app explanations (subtitle definitions) adjacent to all permission toggles.
- [ ] Implement the "View Server As Role" simulator debug switch.
- [ ] Thorough end-to-end testing of the priority model matrix to confirm edge-cases match expected access.
- **Notes**: 
  - *(Add implementation notes here)*
