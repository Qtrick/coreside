import { useEffect, useState } from "react";
import { CheckCircle2, PlugZap } from "lucide-react";
import { api } from "@/lib/tauri";
import type { ProviderConnection } from "@/types/providers";
import { useAppStore } from "@/stores/app-store";
import {
  isHostedAuthConfigured,
  signInWithPassword,
  signUpWithPassword,
} from "@/lib/hosted/supabase-auth";
import { serializeHostedAuthSession } from "@/lib/hosted/auth-session";

/**
 * Consumer AI connections card.
 * Provider/model identity only when disclosure policy allows (user-connected / local / Developer Mode).
 */
export function AiProviderSettings() {
  const aiStatus = useAppStore((s) => s.aiStatus);
  const testingConnection = useAppStore((s) => s.testingConnection);
  const connectionTestMessage = useAppStore((s) => s.connectionTestMessage);
  const testConnection = useAppStore((s) => s.testConnection);
  const openProviderSetup = useAppStore((s) => s.openProviderSetup);
  const refreshAiStatus = useAppStore((s) => s.refreshAiStatus);
  const developerMode = useAppStore((s) => s.developerMode);
  const setDeveloperMode = useAppStore((s) => s.setDeveloperMode);

  const [connections, setConnections] = useState<ProviderConnection[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [managing, setManaging] = useState(false);
  const [hostedConfigured] = useState(() => isHostedAuthConfigured());
  const [hostedSignedIn, setHostedSignedIn] = useState(false);
  const [hostedPlan, setHostedPlan] = useState<{
    planId: string;
    displayName: string;
    hostedAiEnabled: boolean;
    allowanceAmount: number;
  } | null>(null);
  const [hostedPlanCatalog, setHostedPlanCatalog] = useState<
    Array<{
      planId: string;
      displayName: string;
      hostedAiEnabled: boolean;
      allowanceAmount: number;
    }>
  >([]);
  const [authEmail, setAuthEmail] = useState("");
  const [authPassword, setAuthPassword] = useState("");
  const [authBusy, setAuthBusy] = useState(false);
  const [authMessage, setAuthMessage] = useState<string | null>(null);

  const disclosure = aiStatus?.disclosure;
  const accessMode = aiStatus?.accessMode ?? "unavailable";
  const showProvider = Boolean(disclosure?.showProviderIdentity);
  const showModel = Boolean(disclosure?.showModelIdentity);
  const allowTest = Boolean(disclosure?.allowConnectionTest);
  const heading =
    aiStatus?.consumerDisplayName ??
    (accessMode === "user_byok" ? "Your AI connections" : "AI connections");
  const statusLabel =
    aiStatus?.userFacingStatus ??
    (aiStatus?.status === "ready" ? "Connected" : "Unavailable");

  const accessModeLabel =
    accessMode === "coreside_hosted"
      ? "Coreside AI"
      : accessMode === "user_local"
        ? "Local AI"
        : accessMode === "user_byok"
          ? "Your own API key"
          : accessMode === "developer_environment"
            ? "Development access"
            : null;

  const reload = async () => {
    try {
      const rows = await api.listProviderConnections();
      setConnections(rows);
      setListError(null);
    } catch (err) {
      setListError(
        err instanceof Error ? err.message : "Could not load providers.",
      );
    }
  };

  useEffect(() => {
    void api
      .getHostedAuthStatus()
      .then((s) => {
        setHostedSignedIn(Boolean(s.signedIn));
        setHostedPlan(s.plan ?? null);
        setHostedPlanCatalog(s.availablePlans ?? []);
      })
      .catch(() => {
        setHostedSignedIn(false);
        setHostedPlan(null);
        setHostedPlanCatalog([]);
      });
  }, [aiStatus?.accessMode, aiStatus?.status]);

  useEffect(() => {
    if (!(managing || accessMode === "user_byok" || accessMode === "user_local")) {
      return;
    }
    void reload();
  }, [aiStatus?.activeConnectionId, aiStatus?.status, accessMode, managing]);

  const statusClass =
    aiStatus?.status === "ready"
      ? "ready"
      : aiStatus?.status === "missing_key" || aiStatus?.status === "unconfigured"
        ? "warn"
        : "";

  const showByokList =
    managing || accessMode === "user_byok" || accessMode === "user_local";

  const runHostedSignIn = async (mode: "signin" | "signup") => {
    setAuthBusy(true);
    setAuthMessage(null);
    try {
      const session =
        mode === "signin"
          ? await signInWithPassword(authEmail, authPassword)
          : await signUpWithPassword(authEmail, authPassword);
      if (!session) {
        setAuthMessage(
          "Check your email to confirm the account, then sign in.",
        );
        return;
      }
      await api.storeHostedAuthSession(serializeHostedAuthSession(session));
      setHostedSignedIn(true);
      setAuthPassword("");
      await refreshAiStatus();
      setAuthMessage(
        mode === "signin" ? "Signed in to Coreside AI." : "Account created.",
      );
    } catch (err) {
      setAuthMessage(
        err instanceof Error ? err.message : "Authentication failed",
      );
    } finally {
      setAuthBusy(false);
    }
  };

  const runHostedSignOut = async () => {
    setAuthBusy(true);
    setAuthMessage(null);
    try {
      await api.clearHostedAuthSession();
      setHostedSignedIn(false);
      await refreshAiStatus();
      setAuthMessage("Signed out.");
    } catch (err) {
      setAuthMessage(err instanceof Error ? err.message : "Sign-out failed");
    } finally {
      setAuthBusy(false);
    }
  };

  return (
    <section
      className="settings-section settings-section-compact"
      aria-labelledby="ai-access-heading"
    >
      <h3 id="ai-access-heading">{heading}</h3>
      <div className="provider-status-row">
        <span className={`status-pill ${statusClass}`}>
          {aiStatus?.status === "ready" ? (
            <CheckCircle2 size={14} aria-hidden />
          ) : (
            <PlugZap size={14} aria-hidden />
          )}
          {statusLabel}
        </span>
        {accessModeLabel ? (
          <span className="status-pill muted" aria-label="Access type">
            {accessModeLabel}
          </span>
        ) : null}
        {showProvider || showModel ? (
          <div className="provider-status-meta">
            {showProvider ? (
              <div>
                <span className="muted">Active provider</span>
                <strong>{aiStatus?.provider || "—"}</strong>
              </div>
            ) : null}
            {showModel ? (
              <div>
                <span className="muted">Current model</span>
                <strong>{aiStatus?.model || "—"}</strong>
              </div>
            ) : null}
          </div>
        ) : aiStatus?.status === "ready" ? (
          <div className="provider-status-meta">
            <div>
              <span className="muted">Service</span>
              <strong>
                {accessMode === "coreside_hosted"
                  ? "Coreside AI"
                  : "Ready for chat"}
              </strong>
            </div>
          </div>
        ) : null}
      </div>

      {aiStatus?.message &&
      (disclosure?.showCredentialSource || aiStatus.status !== "ready") ? (
        <p className="muted">{aiStatus.message}</p>
      ) : null}

      {hostedConfigured ? (
        <div className="hosted-auth-block">
          <h4 className="settings-subheading hosted-auth-heading">
            Coreside AI account
          </h4>
          {hostedSignedIn ? (
            <div className="hosted-plan-summary">
              {hostedPlan ? (
                <p className="muted" role="status">
                  Plan: <strong>{hostedPlan.displayName}</strong>
                  {hostedPlan.hostedAiEnabled
                    ? ` · ${hostedPlan.allowanceAmount.toLocaleString()} requests / period`
                    : " · Coreside AI not included"}
                </p>
              ) : null}
              {hostedPlanCatalog.length > 0 ? (
                <ul className="hosted-plan-catalog muted" aria-label="Available plans">
                  {hostedPlanCatalog.map((row) => (
                    <li key={row.planId}>
                      {row.displayName}
                      {row.hostedAiEnabled
                        ? ` — ${row.allowanceAmount.toLocaleString()} requests`
                        : " — sign-in only"}
                    </li>
                  ))}
                </ul>
              ) : null}
              <div className="button-row">
                <button
                  type="button"
                  className="btn btn-secondary"
                  disabled={authBusy}
                  onClick={() => void runHostedSignOut()}
                >
                  Sign out
                </button>
              </div>
            </div>
          ) : (
            <form
              className="hosted-auth-form"
              onSubmit={(e) => {
                e.preventDefault();
                if (!authBusy && authEmail.trim() && authPassword) {
                  void runHostedSignIn("signin");
                }
              }}
            >
              <label className="field" htmlFor="hosted-email">
                <span>Email</span>
                <input
                  id="hosted-email"
                  type="email"
                  autoComplete="username"
                  placeholder="you@example.com"
                  value={authEmail}
                  onChange={(e) => setAuthEmail(e.target.value)}
                  disabled={authBusy}
                />
              </label>
              <label className="field" htmlFor="hosted-password">
                <span>Password</span>
                <input
                  id="hosted-password"
                  type="password"
                  autoComplete="current-password"
                  placeholder="Password"
                  value={authPassword}
                  onChange={(e) => setAuthPassword(e.target.value)}
                  disabled={authBusy}
                />
              </label>
              <div className="button-row hosted-auth-actions">
                <button
                  type="submit"
                  className="btn btn-primary"
                  disabled={authBusy || !authEmail.trim() || !authPassword}
                >
                  {authBusy ? "Working…" : "Sign in"}
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  disabled={authBusy || !authEmail.trim() || !authPassword}
                  onClick={() => void runHostedSignIn("signup")}
                >
                  Create account
                </button>
              </div>
              <p className="muted hosted-auth-hint">
                Enter email and password, then sign in or create an account.
              </p>
            </form>
          )}
          {authMessage ? (
            <p className="muted" role="status">
              {authMessage}
            </p>
          ) : null}
        </div>
      ) : (
        <p className="muted" style={{ marginTop: "0.85rem" }}>
          Coreside AI requires a build configured with Supabase. You can still
          connect your own provider below.
        </p>
      )}

      {accessMode === "unavailable" && !hostedConfigured ? (
        <p className="muted">
          Connect your own provider, configure local AI, or use Coreside AI when
          available in this build.
        </p>
      ) : null}

      <div className="hosted-auth-byok">
        <h4 className="settings-subheading hosted-auth-heading">Your own AI</h4>
        <p className="muted hosted-auth-hint">
          Connect a provider API key (BYOK) instead of Coreside AI. Keys stay on
          your device in secure storage — not on Coreside servers.
        </p>
        <div className="button-row">
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => {
              setManaging(true);
              openProviderSetup();
            }}
          >
            {accessMode === "coreside_hosted" ||
            accessMode === "developer_environment"
              ? "Use My Own AI"
              : "Manage providers"}
          </button>
          {allowTest ? (
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => void testConnection()}
              disabled={testingConnection}
            >
              {testingConnection ? "Testing…" : "Check connection"}
            </button>
          ) : null}
        </div>
      </div>
      {connectionTestMessage && allowTest ? (
        <p role="status" className="muted">
          {connectionTestMessage}
        </p>
      ) : null}

      {listError ? (
        <p className="provider-error" role="alert">
          {listError}
        </p>
      ) : null}

      {showByokList ? (
        connections.length > 0 ? (
          <ul className="provider-connection-list">
            {connections.map((conn) => (
              <li key={conn.id} className="provider-connection-item">
                <div>
                  <strong>
                    {conn.label}
                    {conn.isActive ? (
                      <span className="provider-active-tag">Active</span>
                    ) : null}
                  </strong>
                  <p className="muted">
                    {conn.provider}
                    {conn.modelDefault ? ` · ${conn.modelDefault}` : ""}
                    {conn.lastStatus ? ` · ${conn.lastStatus}` : ""}
                  </p>
                </div>
                <div className="button-row">
                  {!conn.isActive ? (
                    <button
                      type="button"
                      className="btn btn-secondary"
                      disabled={busyId === conn.id}
                      onClick={() => {
                        setBusyId(conn.id);
                        void api
                          .setActiveProviderConnection(conn.id)
                          .then(() => refreshAiStatus())
                          .then(() => reload())
                          .finally(() => setBusyId(null));
                      }}
                    >
                      Use
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => openProviderSetup(conn)}
                  >
                    Edit
                  </button>
                  {confirmDeleteId === conn.id ? (
                    <>
                      <button
                        type="button"
                        className="btn btn-danger"
                        disabled={busyId === conn.id}
                        onClick={() => {
                          setBusyId(conn.id);
                          void api
                            .deleteProviderConnection(conn.id)
                            .then(() => refreshAiStatus())
                            .then(() => reload())
                            .finally(() => {
                              setBusyId(null);
                              setConfirmDeleteId(null);
                            });
                        }}
                      >
                        Confirm remove
                      </button>
                      <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={() => setConfirmDeleteId(null)}
                      >
                        Cancel
                      </button>
                    </>
                  ) : (
                    <button
                      type="button"
                      className="btn btn-danger"
                      onClick={() => setConfirmDeleteId(conn.id)}
                    >
                      Remove
                    </button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        ) : managing || accessMode === "user_byok" ? (
          <p className="muted" style={{ marginBottom: 0 }}>
            No saved providers yet. Connect one to use your own AI key.
          </p>
        ) : null
      ) : null}

      <div
        className="button-row"
        style={{ marginTop: "0.85rem", alignItems: "center" }}
      >
        <label
          className="muted"
          style={{ display: "inline-flex", gap: 8, alignItems: "center" }}
        >
          <input
            type="checkbox"
            checked={developerMode}
            onChange={(e) => void setDeveloperMode(e.target.checked)}
          />
          Developer Mode
        </label>
      </div>
      {developerMode && accessMode === "developer_environment" ? (
        <p className="muted" role="status">
          Developer details: credential source is the development environment.
          Upstream routing stays in protected diagnostics only when needed.
        </p>
      ) : null}
    </section>
  );
}
