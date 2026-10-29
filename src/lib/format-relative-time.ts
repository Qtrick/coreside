/**
 * Zero-dependency relative time formatter following Ponytail.
 * Formats ISO strings into concise UI representations (e.g. "2h ago", "1d ago").
 */
export function formatRelativeTime(dateString: string | null | undefined): string {
  if (!dateString) return "";
  try {
    const timestamp = new Date(dateString).getTime();
    if (Number.isNaN(timestamp)) return "";
    const diffMs = Date.now() - timestamp;
    if (diffMs < 0) return "just now";
    const minutes = Math.floor(diffMs / 60_000);
    if (minutes < 1) return "just now";
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    if (days < 30) return `${days}d ago`;
    const months = Math.floor(days / 30);
    if (months < 12) return `${months}mo ago`;
    const years = Math.floor(days / 365);
    return `${years}y ago`;
  } catch {
    return "";
  }
}

