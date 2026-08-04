import { WHATS_NEW_ID } from "./tutorials";

export type WhatsNewItem = {
  title: string;
  body: string;
};

/** Static RC3.4 release notes for Help & learning / upgrade banner. */
export const WHATS_NEW_VERSION_LABEL = "RC3.4";

export const WHATS_NEW_ITEMS: readonly WhatsNewItem[] = [
  {
    title: "Apps terminology",
    body: "Personal tools are called Apps in the sidebar and empty states — clearer for everyday use.",
  },
  {
    title: "Help & learning",
    body: "Restart the essentials tour and open learning modules from Settings → Help & learning.",
  },
  {
    title: "Progressive preview",
    body: "Proposed app changes can appear as a live preview before you accept them.",
  },
  {
    title: "Wallpapers in Appearance",
    body: "Find and apply wallpapers under Appearance — readability stays protected.",
  },
] as const;

export { WHATS_NEW_ID };
