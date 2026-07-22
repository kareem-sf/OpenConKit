// GENERATED FILE - DO NOT EDIT BY HAND.
//
// Mirror of `openconkit_tool_sdk::ToolDescriptor` (crates/openconkit-tool-sdk).
// From the contracts phase on, this file is emitted by ts-rs and
// drift-checked in CI; the shape below is the contract it must match.

import { z } from "zod";

/** Stable metadata describing a hosted tool. */
export interface ToolDescriptor {
  /** Unique kebab-case identifier, e.g. `boq-inspector`. */
  id: string;
  /** Contract version the tool targets. */
  contract_version: number;
  /** i18n key for the tool's display name. */
  name_key: string;
  /** i18n key for the tool's one-line description. */
  description_key: string;
}

/** Runtime validator for {@link ToolDescriptor}. */
export const toolDescriptorSchema: z.ZodType<ToolDescriptor> = z.object({
  id: z.string().regex(/^[a-z0-9]+(?:-[a-z0-9]+)*$/, "tool id must be kebab-case"),
  contract_version: z.number().int().positive(),
  name_key: z.string().min(1),
  description_key: z.string().min(1),
});
