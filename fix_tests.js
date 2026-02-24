const fs = require('fs');

function patch(file, search, replace) {
  if (fs.existsSync(file)) {
    let content = fs.readFileSync(file, 'utf8');
    content = content.replace(search, replace);
    fs.writeFileSync(file, content);
  }
}

patch('apps/filament-client-web/tests/app-shell-panel-host-props.test.tsx',
  /isMutatingMemberRoles: false,/g,
  'isMutatingMemberRoles: false, isLoadingMembers: false, memberListError: "",');

patch('apps/filament-client-web/tests/app-shell-support-panel-host-state-options.test.ts',
  /displayUserLabel: \(\) => "",/g,
  'displayUserLabel: () => "", workspaceMembersByGuildId: () => ({}), isLoadingWorkspaceMembers: () => false, workspaceMembersError: () => "",');

patch('apps/filament-client-web/tests/app-shell-support-panel-prop-groups-options.test.ts',
  /viewAsRoleSimulatorRole: \(\) => "member",/g,
  'viewAsRoleSimulatorRole: () => "member", isLoadingWorkspaceMembers: () => false, workspaceMembersError: () => "",');

patch('apps/filament-client-web/tests/app-shell-support-panel-prop-groups.test.ts',
  /isMutatingMemberRoles: false,/g,
  'isMutatingMemberRoles: false, isLoadingMembers: false, memberListError: "",');

patch('apps/filament-client-web/tests/app-shell-workspace-settings-panel-props.test.ts',
  /isMutatingMemberRoles: false,/g,
  'isMutatingMemberRoles: false, isLoadingMembers: false, memberListError: "",');

patch('apps/filament-client-web/tests/app-shell-workspace-settings-panel.test.tsx',
  /isMutatingMemberRoles: false,/g,
  'isMutatingMemberRoles: false, isLoadingMembers: false, memberListError: "",');

patch('apps/filament-client-web/tests/viewport-height.test.ts',
  /installViewportHeightCssVar\(browserWindow\);/g,
  'installViewportHeightCssVar(browserWindow as any);');

