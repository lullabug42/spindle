import * as z from "zod";

export const serviceMetaSchema = z.object({
    name: z.string(),
    version: z.string(),
    description: z.string(),
    program: z.string(),
    args: z.array(z.string()),
    config_path: z.string(),
    workspace: z.string().optional(),
});

export type ServiceMeta = z.infer<typeof serviceMetaSchema>;

export const serviceTopoSchema = z.object({
    name: z.string(),
    group_idx: z.number(),
    node_idx: z.number(),
    dep_idxs: z.array(z.number()),
});

export type ServiceTopo = z.infer<typeof serviceTopoSchema>;

export const groupServiceInfoSchema = z.object({
    name: z.string(),
    version: z.string(),
    description: z.string(),
    program: z.string(),
    args: z.array(z.string()),
    config_path: z.string(),
    workspace: z.string().optional(),
    group_idx: z.number(),
    node_idx: z.number(),
    dep_idxs: z.array(z.number()),
});

export type GroupServiceInfo = z.infer<typeof groupServiceInfoSchema>;