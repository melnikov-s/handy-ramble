import { z } from "zod";

export const ShortcutBindingSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string(),
  default_binding: z.string(),
  current_binding: z.string(),
});

export const ShortcutBindingsMapSchema = z.record(
  z.string(),
  ShortcutBindingSchema
);

export const SettingsSchema = z.object({
  bindings: ShortcutBindingsMapSchema,
  push_to_talk: z.boolean(),
});

export const BindingResponseSchema = z.object({
  success: z.boolean(),
  binding: ShortcutBindingSchema.nullable(),
  error: z.string().nullable(),
});

export type BindingResponse = z.infer<typeof BindingResponseSchema>;
export type ShortcutBinding = z.infer<typeof ShortcutBindingSchema>;
export type ShortcutBindingsMap = z.infer<typeof ShortcutBindingsMapSchema>;
export type Settings = z.infer<typeof SettingsSchema>;
