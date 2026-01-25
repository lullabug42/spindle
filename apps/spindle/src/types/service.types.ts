import * as z from "zod";

export const serviceConfigSchema = z.object({
    name: z.string(),
    version: z.string(),
    description: z.string(),
    program: z.string(),
    args: z.array(z.string()).default([]),
    dependencies: z.array(z.string()).default([]),
    workspace: z.string().optional(),
});

export type ServiceConfig = z.infer<typeof serviceConfigSchema>;
