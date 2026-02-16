import type { GuildId } from "../../../domain/chat";
import {
  createGatewayController,
  type GatewayControllerOptions,
} from "../controllers/gateway-controller";

export interface GatewayControllerRegistrationOptions
  extends Omit<GatewayControllerOptions, "onWorkspacePermissionsChanged"> {
  refreshWorkspacePermissionStateFromGateway: (
    guildId: GuildId,
  ) => Promise<void>;
  onGatewayConnectionChange?: (isOpen: boolean) => void;
}

export interface GatewayControllerRegistrationDependencies {
  createGatewayController: typeof createGatewayController;
}

const DEFAULT_GATEWAY_CONTROLLER_REGISTRATION_DEPENDENCIES: GatewayControllerRegistrationDependencies =
  {
    createGatewayController,
  };

export function registerGatewayController(
  options: GatewayControllerRegistrationOptions,
  dependencies: Partial<GatewayControllerRegistrationDependencies> = {},
): void {
  const deps = {
    ...DEFAULT_GATEWAY_CONTROLLER_REGISTRATION_DEPENDENCIES,
    ...dependencies,
  };

  deps.createGatewayController({
    ...options,
    onWorkspacePermissionsChanged: (guildId) => {
      void options.refreshWorkspacePermissionStateFromGateway(guildId);
    },
  });
}