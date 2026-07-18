import { useEffect, useId, useRef, useState, type FormEvent } from "react";
import { Eye, EyeOff, X } from "lucide-react";
import { api } from "@/lib/tauri";
import type {
  CacheStats,
  ExaBudgetStatus,
  ExaUsageSummary,
  ResourceProfile,
  SearchConnection,
  SearchProfileView,
  SearchUsageProfile,
} from "@/types/search";
import { SearchHistoryPanel } from "@/components/search/SearchHistoryPanel";
import { ModalPortal } from "@/components/ui/ModalPortal";

type EngineStatusLabel = "Ready" | "Needs Setup" | "Unavailable" | "Error";

function mapEngineStatus(
  connection: SearchConnection | null,
): { label: EngineStatusLabel; className: string } {
  if (!connection) {
    return { label: "Unavailable", className: "warn" };
  }
  if (connection.engineReady) {
    return { label: "Ready", className: "ready" };
  }
  const reason = (connection.engineReason ?? "").toLowerCase();
  if (
    reason.includes("unavailable") ||
    reason.includes("not supported") ||
    reason.includes("unsupported")
  ) {
    return { label: "Unavailable", className: "warn" };
  }
  if (
    reason.includes("error") ||
    reason.includes("failed") ||
    reason.includes("crash") ||
    reason.includes("corrupt")
  ) {
    return { label: "Error", className: "error" };
  }
  return { label: "Needs Setup", className: "warn" };
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 ? 0 : value >= 10 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[unit]}`;
}

const RESOURCE_PROFILES: {
  id: ResourceProfile;
  label: string;
  hint: string;
}[] = [
  { id: "eco", label: "Eco", hint: "Lightest load" },
  { id: "balanced", label: "Balanced", hint: "Default" },
  { id: "performance", label: "Performance", hint: "Faster, more load" },
];

const SEARCH_PROFILES: {
  id: SearchUsageProfile;
  label: string;
  hint: string;
}[] = [
  { id: "saver", label: "Saver", hint: "Default — fewest Exa credits" },
  { id: "balanced", label: "Balanced", hint: "5 results, one refinement" },
  { id: "thorough", label: "Thorough", hint: "More sources, budget-gated deep" },
];

type SetupProps = {
  open: boolean;
  onClose: () => void;
};

/** Setup guidance when the local research engine is not ready. */
export function SearchProviderSetup({ open, onClose }: SetupProps) {
  const titleId = useId();
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const t = window.setTimeout(() => dialogRef.current?.focus(), 0);
    return () => window.clearTimeout(t);
  }, [open]);

  if (!open) return null;

  return (
    <ModalPortal>
      <div className="provider-modal-root" role="presentation">
        <button
          type="button"
          className="provider-modal-backdrop"
          aria-label="Dismiss"
          tabIndex={-1}
          onClick={onClose}
        />
        <div
          ref={dialogRef}
          className="provider-modal"
          role="dialog"
          aria-modal="true"
          aria-labelledby={titleId}
          tabIndex={-1}
        >
          <header className="provider-modal-header">
            <h2 id={titleId}>Set up Web Research</h2>
            <button
              type="button"
              className="btn-icon"
              onClick={onClose}
              aria-label="Close"
            >
              <X size={18} aria-hidden />
            </button>
          </header>
          <div className="provider-key-form">
            <p>
              Indexed web discovery uses Exa (optional API key). Page inspection
              uses the local Crawl4AI engine on this computer.
            </p>
            <p className="muted">Developers: install the local engine with:</p>
            <pre className="settings-code-block">npm run crawl4ai:setup</pre>
            <p className="muted">
              Add <code>EXA_API_KEY=</code> to <code>.env</code> for indexed web
              discovery (development). Crawl4AI remains the local page inspector
              when installed.
            </p>
            <div className="button-row provider-form-actions">
              <button type="button" className="btn btn-primary" onClick={onClose}>
                Got it
              </button>
            </div>
          </div>
        </div>
      </div>
    </ModalPortal>
  );
}

type ExaSetupProps = {
  open: boolean;
  onClose: () => void;
  onSaved?: () => void;
};

function ExaKeySetup({ open, onClose, onSaved }: ExaSetupProps) {
  const titleId = useId();
  const keyInputRef = useRef<HTMLInputElement>(null);
  const [apiKey, setApiKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setApiKey("");
      setShowKey(false);
      setBusy(false);
      setError(null);
      return;
    }
    const t = window.setTimeout(() => keyInputRef.current?.focus(), 0);
    return () => window.clearTimeout(t);
  }, [open]);

  if (!open) return null;

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const trimmed = apiKey.trim();
    if (!trimmed) {
      setError("API key is required.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await api.configureExaConnection(trimmed);
      try {
        await api.testExaConnection();
      } catch (err) {
        setError(
          err instanceof Error
            ? err.message
            : "Key saved but connection test failed.",
        );
        setBusy(false);
        return;
      }
      setApiKey("");
      onSaved?.();
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not save Exa key.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <ModalPortal>
      <div className="provider-modal-root" role="presentation">
        <button
          type="button"
          className="provider-modal-backdrop"
          aria-label="Dismiss"
          tabIndex={-1}
          disabled={busy}
          onClick={() => {
            if (!busy) onClose();
          }}
        />
        <div
          className="provider-modal"
          role="dialog"
          aria-modal="true"
          aria-labelledby={titleId}
        >
          <header className="provider-modal-header">
            <h2 id={titleId}>Configure Exa</h2>
            <button
              type="button"
              className="btn-icon"
              onClick={() => {
                if (!busy) onClose();
              }}
              aria-label="Close"
              disabled={busy}
            >
              <X size={18} aria-hidden />
            </button>
          </header>
          <form className="provider-key-form" onSubmit={(e) => void onSubmit(e)}>
            <p>
              Exa powers indexed web discovery. Keys are stored in the OS
              keyring and never shown again. Crawl4AI remains the local page
              inspector.
            </p>
            <label className="field">
              <span>Exa API key</span>
              <div className="provider-key-input-row">
                <input
                  ref={keyInputRef}
                  type={showKey ? "text" : "password"}
                  autoComplete="off"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  disabled={busy}
                />
                <button
                  type="button"
                  className="btn-icon"
                  aria-label={showKey ? "Hide key" : "Show key"}
                  onClick={() => setShowKey((v) => !v)}
                  disabled={busy}
                >
                  {showKey ? <EyeOff size={16} /> : <Eye size={16} />}
                </button>
              </div>
            </label>
            {error ? <p className="provider-error">{error}</p> : null}
            <div className="button-row provider-form-actions">
              <button
                type="button"
                className="btn btn-secondary"
                disabled={busy}
                onClick={onClose}
              >
                Cancel
              </button>
              <button type="submit" className="btn btn-primary" disabled={busy}>
                Save and test
              </button>
            </div>
          </form>
        </div>
      </div>
    </ModalPortal>
  );
}

export function SearchSettingsSection() {
  const [connection, setConnection] = useState<SearchConnection | null>(null);
  const [safeSearch, setSafeSearch] = useState<"strict" | "standard" | "off">(
    "standard",
  );
  const [resourceProfile, setResourceProfile] =
    useState<ResourceProfile>("balanced");
  const [searchProfile, setSearchProfile] = useState<SearchProfileView | null>(
    null,
  );
  const [usage, setUsage] = useState<ExaUsageSummary | null>(null);
  const [budget, setBudget] = useState<ExaBudgetStatus | null>(null);
  const [budgetInput, setBudgetInput] = useState("");
  const [cacheStats, setCacheStats] = useState<CacheStats | null>(null);
  const [setupOpen, setSetupOpen] = useState(false);
  const [exaSetupOpen, setExaSetupOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [testMessage, setTestMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmClearCache, setConfirmClearCache] = useState(false);

  const reload = async () => {
    try {
      const [conn, settings, status, cache, profile, usageSum, budgetStatus] =
        await Promise.all([
          api.getSearchConnection(),
          api.getSettings(),
          api.getCrawlerStatus().catch(() => null),
          api.getCrawlerCacheStats().catch(() => null),
          api.getSearchProfile().catch(() => null),
          api.getExaUsage().catch(() => null),
          api.getExaBudget().catch(() => null),
        ]);
      setConnection(conn);
      setSafeSearch(settings.safeSearch ?? "standard");
      if (status?.resourceProfile) {
        const profileId = status.resourceProfile.toLowerCase();
        if (
          profileId === "eco" ||
          profileId === "balanced" ||
          profileId === "performance"
        ) {
          setResourceProfile(profileId);
        }
      }
      setCacheStats(cache);
      setSearchProfile(profile);
      setUsage(usageSum);
      setBudget(budgetStatus);
      setBudgetInput(
        budgetStatus?.budgetUsd != null && Number.isFinite(budgetStatus.budgetUsd)
          ? String(budgetStatus.budgetUsd)
          : "",
      );
      setError(null);
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : "Could not load Web Research settings.",
      );
    }
  };

  useEffect(() => {
    void reload();
  }, []);

  const status = mapEngineStatus(connection);
  const needsSetup = status.label === "Needs Setup";
  const exaConfigured = Boolean(connection?.exaConfigured);

  const testConnection = async () => {
    setBusy(true);
    setTestMessage(null);
    try {
      const msg = await api.testSearchConnection();
      setTestMessage(msg);
      await reload();
    } catch (err) {
      setTestMessage(
        err instanceof Error ? err.message : "Research engine check failed.",
      );
    } finally {
      setBusy(false);
    }
  };

  const testExa = async () => {
    setBusy(true);
    setTestMessage(null);
    try {
      const msg = await api.testExaConnection();
      setTestMessage(msg);
      await reload();
    } catch (err) {
      setTestMessage(err instanceof Error ? err.message : "Exa test failed.");
    } finally {
      setBusy(false);
    }
  };

  const updateSafeSearch = async (value: "strict" | "standard" | "off") => {
    setSafeSearch(value);
    try {
      await api.setSetting("safeSearch", value);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Could not update safe search.",
      );
    }
  };

  const updateResourceProfile = async (profile: ResourceProfile) => {
    setResourceProfile(profile);
    setBusy(true);
    try {
      const saved = await api.setWebResearchResourceProfile(profile);
      if (saved === "eco" || saved === "balanced" || saved === "performance") {
        setResourceProfile(saved);
      }
      setError(null);
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : "Could not update resource profile.",
      );
      await reload();
    } finally {
      setBusy(false);
    }
  };

  const updateSearchProfile = async (profile: SearchUsageProfile) => {
    setBusy(true);
    try {
      const view = await api.setSearchProfile(profile);
      setSearchProfile(view);
      setError(null);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Could not update search profile.",
      );
    } finally {
      setBusy(false);
    }
  };

  const saveBudget = async () => {
    setBusy(true);
    try {
      const trimmed = budgetInput.trim();
      const monthly = trimmed === "" ? null : Number.parseFloat(trimmed);
      if (trimmed !== "" && (!Number.isFinite(monthly) || (monthly ?? 0) <= 0)) {
        setError("Enter a positive monthly budget in USD, or leave empty.");
        setBusy(false);
        return;
      }
      const next = await api.setExaBudget({ monthlyBudgetUsd: monthly });
      setBudget(next);
      setBudgetInput(
        next.budgetUsd != null && Number.isFinite(next.budgetUsd)
          ? String(next.budgetUsd)
          : "",
      );
      setError(null);
      setTestMessage("Local Exa budget updated.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not save budget.");
    } finally {
      setBusy(false);
    }
  };

  const clearCache = async () => {
    setBusy(true);
    try {
      await api.cleanupCrawlerCache();
      setConfirmClearCache(false);
      const stats = await api.getCrawlerCacheStats().catch(() => null);
      setCacheStats(stats);
      setTestMessage("Research cache cleared.");
      setError(null);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Could not clear research cache.",
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <section
        className="settings-section settings-section-compact"
        aria-labelledby="search-media-heading"
      >
        <h3 id="search-media-heading">Web Search and Research</h3>
        <p>
          Exa finds and ranks web sources. Crawl4AI inspects selected pages
          locally. Saver mode is the default so your Exa allowance lasts.
        </p>

        <h4 className="settings-subheading">Indexed search (Exa)</h4>
        <div className="provider-status-row">
          <span className={`status-pill ${exaConfigured ? "ready" : "warn"}`}>
            {exaConfigured ? "Connected" : "Not configured"}
          </span>
          <div className="provider-status-meta">
            <div>
              <span className="muted">Credential</span>
              <strong>{connection?.exaSource ?? "none"}</strong>
            </div>
            <div>
              <span className="muted">Month usage</span>
              <strong>
                {usage
                  ? `$${usage.actualCostTotal.toFixed(4)} · ${usage.requestCount} req`
                  : "—"}
              </strong>
            </div>
          </div>
        </div>
        {budget?.note ? <p className="muted">{budget.note}</p> : null}
        {budget?.threshold && budget.threshold !== "ok" ? (
          <p className="muted" role="status">
            Local budget threshold: {budget.threshold}
            {budget.percentUsed != null
              ? ` (${budget.percentUsed.toFixed(0)}%)`
              : ""}
          </p>
        ) : null}

        <div className="button-row">
          <button
            type="button"
            className="btn btn-primary"
            disabled={busy}
            onClick={() => setExaSetupOpen(true)}
          >
            {exaConfigured ? "Replace Exa key" : "Add Exa key"}
          </button>
          <button
            type="button"
            className="btn btn-secondary"
            disabled={busy || !exaConfigured}
            onClick={() => void testExa()}
          >
            Test Exa
          </button>
          {exaConfigured ? (
            <button
              type="button"
              className="btn btn-secondary"
              disabled={busy}
              onClick={() => {
                void (async () => {
                  setBusy(true);
                  try {
                    await api.deleteExaConnection();
                    await reload();
                    setTestMessage("Exa key removed from keyring.");
                  } catch (err) {
                    setError(
                      err instanceof Error
                        ? err.message
                        : "Could not remove Exa key.",
                    );
                  } finally {
                    setBusy(false);
                  }
                })();
              }}
            >
              Remove key
            </button>
          ) : null}
        </div>

        <h4 className="settings-subheading">Search usage profile</h4>
        <p className="muted">
          Controls Exa result counts and Crawl4AI handoff limits. Default is
          Saver.
        </p>
        <div
          className="theme-options"
          role="group"
          aria-label="Search usage profile"
        >
          {SEARCH_PROFILES.map((profile) => (
            <button
              key={profile.id}
              type="button"
              className="btn btn-secondary"
              aria-pressed={searchProfile?.profile === profile.id}
              title={profile.hint}
              disabled={busy}
              onClick={() => void updateSearchProfile(profile.id)}
            >
              {profile.label}
            </button>
          ))}
        </div>
        {searchProfile ? (
          <p className="muted">
            Mode {searchProfile.searchType} · {searchProfile.numResults} results
            · ≤{searchProfile.maxCrawlPages} local pages
          </p>
        ) : null}

        <h4 className="settings-subheading">Local monthly budget (optional)</h4>
        <p className="muted">
          Caps automatic Exa spend in Coreside. This is not your Exa account
          balance.
        </p>
        <div className="button-row" style={{ alignItems: "center", gap: "0.5rem" }}>
          <label className="field" style={{ flex: 1, margin: 0 }}>
            <span className="sr-only">Monthly budget USD</span>
            <input
              type="number"
              min={0}
              step={0.5}
              placeholder="USD / month (empty = unlimited)"
              value={budgetInput}
              onChange={(e) => setBudgetInput(e.target.value)}
              disabled={busy}
            />
          </label>
          <button
            type="button"
            className="btn btn-secondary"
            disabled={busy}
            onClick={() => void saveBudget()}
          >
            Save budget
          </button>
        </div>

        <h4 className="settings-subheading">Local crawl engine</h4>
        <p className="muted">
          Crawl4AI (internal) — page extraction, robots, and media discovery
        </p>

        <div className="provider-status-row">
          <span className={`status-pill ${status.className}`}>
            {status.label}
          </span>
          <div className="provider-status-meta">
            <div>
              <span className="muted">Source</span>
              <strong>{connection?.source ?? "none"}</strong>
            </div>
          </div>
        </div>

        {connection?.engineReason && !connection.engineReady ? (
          <p className="muted" role="status">
            {connection.engineReason}
          </p>
        ) : null}

        {needsSetup ? (
          <p>
            The local crawl engine is not installed yet. Developers should run{" "}
            <code>npm run crawl4ai:setup</code>, then reopen Settings.
          </p>
        ) : null}

        {error ? <p className="provider-error">{error}</p> : null}
        {testMessage ? <p className="muted">{testMessage}</p> : null}

        <div className="button-row">
          {needsSetup ? (
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => setSetupOpen(true)}
              disabled={busy}
            >
              Setup guidance
            </button>
          ) : null}
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => void testConnection()}
            disabled={busy || !connection?.engineReady}
          >
            Test crawl engine
          </button>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => void reload()}
            disabled={busy}
          >
            Refresh status
          </button>
        </div>

        <h4 className="settings-subheading">Resource profile</h4>
        <p className="muted">
          How hard the local crawl engine works on this computer.
        </p>
        <div
          className="theme-options"
          role="group"
          aria-label="Web Research resource profile"
        >
          {RESOURCE_PROFILES.map((profile) => (
            <button
              key={profile.id}
              type="button"
              className="btn btn-secondary"
              aria-pressed={resourceProfile === profile.id}
              title={profile.hint}
              disabled={busy}
              onClick={() => void updateResourceProfile(profile.id)}
            >
              {profile.label}
            </button>
          ))}
        </div>

        <h4 className="settings-subheading">Research cache</h4>
        <p className="muted">
          Cache size:{" "}
          <strong>
            {cacheStats ? formatBytes(cacheStats.sizeBytes) : "—"}
          </strong>
          {cacheStats?.quotaBytes
            ? ` of ${formatBytes(cacheStats.quotaBytes)} quota`
            : null}
          {cacheStats?.overQuota ? " (over quota)" : null}
        </p>
        <div className="button-row">
          {!confirmClearCache ? (
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => setConfirmClearCache(true)}
              disabled={busy || !connection?.engineReady}
            >
              Clear research cache
            </button>
          ) : (
            <>
              <button
                type="button"
                className="btn btn-danger"
                onClick={() => void clearCache()}
                disabled={busy}
              >
                Confirm clear
              </button>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => setConfirmClearCache(false)}
                disabled={busy}
              >
                Cancel
              </button>
            </>
          )}
        </div>

        <h4 className="settings-subheading">Safe search</h4>
        <div className="theme-options" role="group" aria-label="Safe search level">
          {(["strict", "standard", "off"] as const).map((level) => (
            <button
              key={level}
              type="button"
              className="btn btn-secondary"
              aria-pressed={safeSearch === level}
              onClick={() => void updateSafeSearch(level)}
            >
              {level === "off" ? "Off" : level[0].toUpperCase() + level.slice(1)}
            </button>
          ))}
        </div>

        <p className="muted settings-compliance-note">
          Coreside respects site restrictions and may be unable to access some
          sources.
        </p>

        <SearchHistoryPanel />
      </section>

      <SearchProviderSetup open={setupOpen} onClose={() => setSetupOpen(false)} />
      <ExaKeySetup
        open={exaSetupOpen}
        onClose={() => setExaSetupOpen(false)}
        onSaved={() => void reload()}
      />
    </>
  );
}
