/** Shared project glyph class. Lives outside the list component so both stay
 * fast-refreshable. */
export function projectIconClass(iconKey?: string | null): string {
  const key = iconKey?.trim() || "folder";
  return `project-icon project-icon-${key}`;
}
