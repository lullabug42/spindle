import { invoke } from "@tauri-apps/api/core";
import type {
  AddServiceParams,
  ServiceNameVersionParams,
  InsertGroupAliasParams,
  GroupIdParams,
  LaunchGroupParams,
  GroupInfo,
} from "../types/service.types";

/** Tauri command names for service-related backend handlers. */
const CMD = {
  add_service: "add_service",
  remove_service: "remove_service",
  reload_service_manager: "reload_service_manager",
  update_service_group_membership: "update_service_group_membership",
  insert_group_alias: "insert_group_alias",
  query_group_alias: "query_group_alias",
  remove_group_alias: "remove_group_alias",
  launch_group: "launch_group",
  stop_service: "stop_service",
  service_state: "service_state",
  stop_group: "stop_group",
  aliased_group_service: "aliased_group_service",
  unaliased_group_service: "unaliased_group_service",
} as const;

/**
 * Adds a service and persists it to the database.
 *
 * @param params - Validated by {@link AddServiceParams}; use {@link addServiceParamsSchema} to parse.
 * @returns The newly inserted service id (database primary key).
 * @throws Rejects with an error message if a dependency is not found or DB insert fails.
 */
export function addService(params: AddServiceParams): Promise<number> {
  return invoke<number>(CMD.add_service, {
    name: params.name,
    version: params.version,
    program: params.program,
    description: params.description ?? null,
    workspace: params.workspace ?? null,
    args: params.args,
    dependencies: params.dependencies,
  });
}

/**
 * Removes a service by name and version (idempotent if not found).
 *
 * @param params - Service identifier; see {@link ServiceNameVersionParams}.
 * @returns Resolves when the service is removed or when no such service exists.
 * @throws Rejects with an error message if removal fails.
 */
export function removeService(params: ServiceNameVersionParams): Promise<void> {
  return invoke<void>(CMD.remove_service, {
    name: params.name,
    version: params.version,
  });
}

/**
 * Reloads all service configs from the database and rebuilds the in-memory ServiceManager.
 *
 * @returns Resolves when the manager has been rebuilt and app state updated.
 * @throws Rejects with an error message if loading or building the manager fails.
 */
export function reloadServiceManager(): Promise<void> {
  return invoke<void>(CMD.reload_service_manager);
}

/**
 * Writes the current ServiceManager group membership and aliases back to the database.
 *
 * @returns Resolves when the DB has been updated.
 * @throws Rejects with an error message if the update fails.
 */
export function updateServiceGroupMembership(): Promise<void> {
  return invoke<void>(CMD.update_service_group_membership);
}

/**
 * Sets the alias for a group.
 *
 * @param params - Group id and alias string; see {@link InsertGroupAliasParams}.
 * @returns Resolves when the alias is stored.
 * @throws Rejects with an error message if the operation fails.
 */
export function insertGroupAlias(params: InsertGroupAliasParams): Promise<void> {
  return invoke<void>(CMD.insert_group_alias, {
    groupId: params.group_id,
    alias: params.alias,
  });
}

/**
 * Returns the alias for the given group, if set.
 *
 * @param groupId - The group id to look up.
 * @returns The alias string, or `null` if not set or on error.
 */
export function queryGroupAlias(groupId: number): Promise<string | null> {
  return invoke<string | null>(CMD.query_group_alias, { groupId });
}

/**
 * Removes the alias for the given group.
 *
 * @param params - Group id whose alias to remove; see {@link GroupIdParams}.
 * @returns Resolves when the alias is removed.
 * @throws Rejects with an error message if the operation fails.
 */
export function removeGroupAlias(params: GroupIdParams): Promise<void> {
  return invoke<void>(CMD.remove_group_alias, {
    groupId: params.group_id,
  });
}

/**
 * Launches all services in the given group (starts in dependency order).
 *
 * @param params - Group id and timeout in ms; see {@link LaunchGroupParams}.
 * @returns Resolves when all services in the group have reached Running or timeout.
 * @throws Rejects if the service manager is not initialized, group id is invalid, or launch fails.
 */
export function launchGroup(params: LaunchGroupParams): Promise<void> {
  return invoke<void>(CMD.launch_group, {
    groupId: params.group_id,
    timeoutMs: params.timeout_ms,
  });
}

/**
 * Stops a service by name and version.
 *
 * @param params - Service identifier; see {@link ServiceNameVersionParams}.
 * @returns Resolves when the service has been stopped.
 * @throws Rejects if the service manager is not initialized or stop fails.
 */
export function stopService(params: ServiceNameVersionParams): Promise<void> {
  return invoke<void>(CMD.stop_service, {
    name: params.name,
    version: params.version,
  });
}

/**
 * Returns the current state of a service (e.g. "Running", "Stopped").
 *
 * @param params - Service identifier; see {@link ServiceNameVersionParams}.
 * @returns The state string for the service.
 * @throws Rejects if the service manager is not initialized or the service is not found.
 */
export function serviceState(params: ServiceNameVersionParams): Promise<string> {
  return invoke<string>(CMD.service_state, {
    name: params.name,
    version: params.version,
  });
}

/**
 * Stops all services in the given group (stops in-degree-0 services; backend cascades to dependents).
 *
 * @param params - Group id; see {@link GroupIdParams}.
 * @returns Resolves when the group has been stopped.
 * @throws Rejects if the service manager is not initialized, group id is invalid, or stop fails.
 */
export function stopGroup(params: GroupIdParams): Promise<void> {
  return invoke<void>(CMD.stop_group, {
    groupId: params.group_id,
  });
}

/**
 * Returns all groups that have an alias set.
 *
 * @returns Array of {@link GroupInfo} for each aliased group.
 */
export function aliasedGroupService(): Promise<GroupInfo[]> {
  return invoke<GroupInfo[]>(CMD.aliased_group_service);
}

/**
 * Returns all groups that have no alias set.
 *
 * @returns Array of {@link GroupInfo} for each unaliased group.
 */
export function unaliasedGroupService(): Promise<GroupInfo[]> {
  return invoke<GroupInfo[]>(CMD.unaliased_group_service);
}
