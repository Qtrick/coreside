import { getSupabaseConfig } from "./supabase-config";
import type { HostedAuthSession } from "./auth-session";

interface TokenResponse {
  access_token?: string;
  refresh_token?: string;
  expires_in?: number;
  expires_at?: number;
  user?: { id?: string };
  error?: string;
  error_description?: string;
  msg?: string;
}

function mapAuthError(payload: TokenResponse, fallback: string): Error {
  const message =
    payload.error_description ||
    payload.msg ||
    payload.error ||
    fallback;
  return new Error(message);
}

function toSession(payload: TokenResponse): HostedAuthSession {
  if (
    !payload.access_token ||
    !payload.refresh_token ||
    !payload.user?.id
  ) {
    throw new Error("Invalid authentication response");
  }
  const expiresAt =
    typeof payload.expires_at === "number"
      ? payload.expires_at * 1000
      : Date.now() + (payload.expires_in ?? 3600) * 1000;
  return {
    accessToken: payload.access_token,
    refreshToken: payload.refresh_token,
    expiresAt,
    userId: payload.user.id,
  };
}

/** Email/password sign-in against Supabase Auth (publishable key only). */
export async function signInWithPassword(
  email: string,
  password: string,
): Promise<HostedAuthSession> {
  const config = getSupabaseConfig();
  if (!config) {
    throw new Error(
      "Coreside AI is not configured in this build (missing Supabase URL).",
    );
  }
  const response = await fetch(`${config.url}/auth/v1/token?grant_type=password`, {
    method: "POST",
    headers: {
      apikey: config.publishableKey,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ email: email.trim(), password }),
  });
  const payload = (await response.json()) as TokenResponse;
  if (!response.ok) {
    throw mapAuthError(payload, "Sign-in failed");
  }
  return toSession(payload);
}

export async function signUpWithPassword(
  email: string,
  password: string,
): Promise<HostedAuthSession | null> {
  const config = getSupabaseConfig();
  if (!config) {
    throw new Error(
      "Coreside AI is not configured in this build (missing Supabase URL).",
    );
  }
  const response = await fetch(`${config.url}/auth/v1/signup`, {
    method: "POST",
    headers: {
      apikey: config.publishableKey,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ email: email.trim(), password }),
  });
  const payload = (await response.json()) as TokenResponse;
  if (!response.ok) {
    throw mapAuthError(payload, "Sign-up failed");
  }
  // Email confirmation may omit tokens until verified.
  if (!payload.access_token) {
    return null;
  }
  return toSession(payload);
}

export function isHostedAuthConfigured(): boolean {
  return getSupabaseConfig() !== null;
}
