export interface HostedAuthSession {
  accessToken: string;
  refreshToken: string;
  expiresAt: number;
  userId: string;
}

export function isSessionExpired(
  session: HostedAuthSession,
  nowMs = Date.now(),
): boolean {
  return nowMs >= session.expiresAt;
}

export function parseHostedAuthSession(raw: unknown): HostedAuthSession | null {
  if (!raw || typeof raw !== "object") return null;
  const value = raw as Record<string, unknown>;
  if (
    typeof value.accessToken !== "string" ||
    typeof value.refreshToken !== "string" ||
    typeof value.expiresAt !== "number" ||
    typeof value.userId !== "string"
  ) {
    return null;
  }
  if (
    !value.accessToken ||
    !value.refreshToken ||
    !value.userId ||
    !Number.isFinite(value.expiresAt)
  ) {
    return null;
  }
  return {
    accessToken: value.accessToken,
    refreshToken: value.refreshToken,
    expiresAt: value.expiresAt,
    userId: value.userId,
  };
}

export function serializeHostedAuthSession(
  session: HostedAuthSession,
): string {
  return JSON.stringify(session);
}

export function deserializeHostedAuthSession(
  raw: string,
): HostedAuthSession | null {
  try {
    return parseHostedAuthSession(JSON.parse(raw));
  } catch {
    return null;
  }
}
