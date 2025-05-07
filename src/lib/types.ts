import { z } from "zod";

export const ShortcutBindingSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string(),
  default_binding: z.string(),
  current_binding: z.string(),
});

export const SettingsSchema = z.object({
  bindings: z.array(ShortcutBindingSchema),
});

export type ShortcutBinding = z.infer<typeof ShortcutBindingSchema>;
export type Settings = z.infer<typeof SettingsSchema>;
