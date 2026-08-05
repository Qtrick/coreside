import { WHATS_NEW_ID } from "./tutorials";

export type WhatsNewItem = {
  title: string;
  body: string;
};

/** Static RC3.6 release notes for Help & learning / upgrade banner. */
export const WHATS_NEW_VERSION_LABEL = "RC3.6";

export const WHATS_NEW_ITEMS: readonly WhatsNewItem[] = [
  {
    title: "AI connections",
    body: "Settings → AI connections explains Coreside AI, Local AI, and bring-your-own-key — with a dedicated tour under Help & learning.",
  },
  {
    title: "Multimodal send safety",
    body: "Image attachments fail closed when bytes are missing instead of silently skipping on providers that claim multimodal support.",
  },
  {
    title: "Progressive app preview",
    body: "Generated-app-capable providers feed live NDJSON operation previews during streaming — with typed parser errors when frames are invalid.",
  },
  {
    title: "Provider tool results",
    body: "OpenAI-family adapters send sealed tool-result envelopes with native role=tool messages instead of flattening trust into user text.",
  },
] as const;

export { WHATS_NEW_ID };
