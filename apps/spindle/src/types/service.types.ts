import { z } from "zod";

/**
 * Zod schema for a dependency entry as `[name, version]`.
 *
 * @remarks
 * Matches backend `Vec<(String, String)>`. Use with `addServiceParamsSchema.dependencies`.
 */
export const serviceDependencySchema = z.tuple([z.string(), z.string()]);

/** Inferred type from {@link serviceDependencySchema}. */
export type ServiceDependency = z.infer<typeof serviceDependencySchema>;

/**
 * Zod schema for parameters when adding a service.
 *
 * @remarks
 * Validates payload for the `add_service` Tauri command.
 */
export const addServiceParamsSchema = z.object({
  name: z.string(),
  version: z.string(),
  program: z.string(),
  description: z.string().nullish(),
  workspace: z.string().nullish(),
  args: z.array(z.string()),
  dependencies: z.array(serviceDependencySchema),
});

/** Inferred type from {@link addServiceParamsSchema}. */
export type AddServiceParams = z.infer<typeof addServiceParamsSchema>;

/**
 * Zod schema for identifying a service by name and version.
 *
 * @remarks
 * Used by `remove_service`, `stop_service`, and `service_state` commands.
 */
export const serviceNameVersionParamsSchema = z.object({
  name: z.string(),
  version: z.string(),
});

/** Inferred type from {@link serviceNameVersionParamsSchema}. */
export type ServiceNameVersionParams = z.infer<
  typeof serviceNameVersionParamsSchema
>;

/**
 * Zod schema for setting a group alias.
 *
 * @remarks
 * Validates payload for the `insert_group_alias` Tauri command.
 */
export const insertGroupAliasParamsSchema = z.object({
  group_id: z.number(),
  alias: z.string(),
});

/** Inferred type from {@link insertGroupAliasParamsSchema}. */
export type InsertGroupAliasParams = z.infer<
  typeof insertGroupAliasParamsSchema
>;

/**
 * Zod schema for operations that take only a group id.
 *
 * @remarks
 * Used by `query_group_alias` and `remove_group_alias` (group_id only).
 */
export const groupIdParamsSchema = z.object({
  group_id: z.number(),
});

/** Inferred type from {@link groupIdParamsSchema}. */
export type GroupIdParams = z.infer<typeof groupIdParamsSchema>;

/**
 * Zod schema for launching a group of services.
 *
 * @remarks
 * `group_id` is `usize` on the backend and `number` on the frontend.
 * Validates payload for the `launch_group` Tauri command.
 */
export const launchGroupParamsSchema = z.object({
  group_id: z.number(),
  timeout_ms: z.number(),
});

/** Inferred type from {@link launchGroupParamsSchema}. */
export type LaunchGroupParams = z.infer<typeof launchGroupParamsSchema>;
