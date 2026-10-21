/**
 * Trusted declarative metrics over queried local_data records.
 *
 * Prefer server-side aggregates / totalCount from local_data.query when present.
 * Fall back to hydrated page arrays only for static/in-memory tables.
 */

export type RecordMetric =
  | { op: "count" }
  | { op: "countWhere"; field: string; equals: string }
  | { op: "countWhere"; field: string; notEquals: string }
  | {
      op: "countOverdue";
      field: string;
      /** When set, records with this status are excluded (e.g. done). */
      statusField?: string;
      excludeStatus?: string;
    };

export type RecordSummarySpec = {
  /** Placeholders `{name}` replaced with metric results. */
  template: string;
  metrics: Record<string, RecordMetric>;
  /** Shown when total count is 0 (optional). */
  emptyText?: string;
  /**
   * Legacy truncated-page hint. Ignored when the payload includes authoritative
   * `totalCount` or `aggregates` from local_data.query.
   */
  queryLimit?: number;
};

const MAX_FIELD_LEN = 64;
const MAX_METRICS = 16;
const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;

/** Unwrap `{ records }` / array payloads the same way DataTable does. */
export function unwrapRecords(value: unknown): Record<string, unknown>[] {
  if (Array.isArray(value)) {
    return value.filter((r): r is Record<string, unknown> => !!r && typeof r === "object");
  }
  if (value && typeof value === "object") {
    const obj = value as Record<string, unknown>;
    for (const key of ["records", "rows", "items", "data"] as const) {
      if (Array.isArray(obj[key])) {
        return (obj[key] as unknown[]).filter(
          (r): r is Record<string, unknown> => !!r && typeof r === "object",
        );
      }
    }
  }
  return [];
}

/** Authoritative total from a query payload (not the page length). */
export function authoritativeTotalCount(value: unknown): number | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const obj = value as Record<string, unknown>;
  const total = obj.totalCount ?? obj.total_count;
  if (typeof total === "number" && Number.isFinite(total) && total >= 0) {
    return Math.floor(total);
  }
  return null;
}

/** Server-computed aggregates map from local_data.query. */
export function authoritativeAggregates(
  value: unknown,
): Record<string, number> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const raw = (value as Record<string, unknown>).aggregates;
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const out: Record<string, number> = {};
  for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
    if (typeof v === "number" && Number.isFinite(v) && v >= 0) {
      out[k] = Math.floor(v);
    }
  }
  return Object.keys(out).length > 0 ? out : null;
}

/** Local calendar date as YYYY-MM-DD — date-only, no timezone shift. */
export function localTodayIsoDate(now: Date = new Date()): string {
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

function safeField(field: unknown): string | null {
  if (typeof field !== "string") return null;
  const f = field.trim();
  if (!f || f.length > MAX_FIELD_LEN || f.startsWith("_")) return null;
  if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(f)) return null;
  return f;
}

function evaluateMetric(
  records: Record<string, unknown>[],
  metric: RecordMetric,
  today: string,
): number {
  switch (metric.op) {
    case "count":
      return records.length;
    case "countWhere": {
      const field = safeField(metric.field);
      if (!field) return 0;
      if ("equals" in metric && typeof metric.equals === "string") {
        const expected = metric.equals;
        return records.filter((r) => String(r[field] ?? "") === expected).length;
      }
      if ("notEquals" in metric && typeof metric.notEquals === "string") {
        const unexpected = metric.notEquals;
        return records.filter((r) => String(r[field] ?? "") !== unexpected).length;
      }
      return 0;
    }
    case "countOverdue": {
      const field = safeField(metric.field);
      if (!field) return 0;
      const statusField = metric.statusField ? safeField(metric.statusField) : null;
      const exclude = typeof metric.excludeStatus === "string" ? metric.excludeStatus : null;
      return records.filter((r) => {
        if (statusField && exclude && String(r[statusField] ?? "") === exclude) {
          return false;
        }
        const raw = r[field];
        if (typeof raw !== "string") return false;
        const date = raw.trim().slice(0, 10);
        if (!ISO_DATE.test(date)) return false;
        return date < today;
      }).length;
    }
    default:
      return 0;
  }
}

function metricAggregateKey(name: string, metric: RecordMetric): string | null {
  // Prefer exact metric name match in aggregates payload.
  if (name) return name;
  switch (metric.op) {
    case "count":
      return "total";
    default:
      return null;
  }
}

function isMetric(value: unknown): value is RecordMetric {
  if (!value || typeof value !== "object") return false;
  const op = (value as { op?: unknown }).op;
  return (
    op === "count" ||
    op === "countWhere" ||
    op === "countOverdue"
  );
}

function parseQueryLimit(value: unknown): number | undefined {
  if (typeof value === "number" && Number.isFinite(value) && value > 0) {
    return Math.floor(value);
  }
  if (typeof value === "string" && value.trim()) {
    const n = Number(value);
    if (Number.isFinite(n) && n > 0) return Math.floor(n);
  }
  return undefined;
}

/**
 * Format a count for display. When the hydrated page is full (`rows.length`
 * equals `queryLimit`) and no authoritative total exists, append `+`.
 */
export function formatCountDisplay(
  count: number,
  rowsLength: number,
  queryLimit?: number,
  authoritative?: boolean,
): string {
  if (authoritative) return String(count);
  if (queryLimit && queryLimit > 0 && rowsLength >= queryLimit) {
    return `${count}+`;
  }
  return String(count);
}

/**
 * Format a declarative summary from queried records.
 * Unknown metrics / malformed specs fall back to emptyText or "".
 * Missing hydration (`null`/`undefined`) returns null so callers keep static text.
 */
export function formatRecordSummary(
  rowsValue: unknown,
  spec: unknown,
  now: Date = new Date(),
): string | null {
  // Missing hydration is not an empty dataset — keep the static prop fallback.
  if (rowsValue === undefined || rowsValue === null) return null;
  if (!spec || typeof spec !== "object") return null;
  const s = spec as Partial<RecordSummarySpec>;
  if (typeof s.template !== "string" || !s.template.trim()) return null;
  if (!s.metrics || typeof s.metrics !== "object") return null;

  const records = unwrapRecords(rowsValue).slice(0, 500);
  const queryLimit = parseQueryLimit(s.queryLimit);
  const serverTotal = authoritativeTotalCount(rowsValue);
  const serverAggs = authoritativeAggregates(rowsValue);
  const authoritative = serverTotal != null || serverAggs != null;
  const today = localTodayIsoDate(now);
  const entries = Object.entries(s.metrics).slice(0, MAX_METRICS);
  const values: Record<string, string> = {};
  for (const [name, metric] of entries) {
    if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name) || !isMetric(metric)) continue;
    let n: number;
    if (serverAggs && typeof serverAggs[name] === "number") {
      n = serverAggs[name]!;
    } else if (metric.op === "count" && serverTotal != null) {
      n = serverTotal;
    } else if (authoritative && serverAggs) {
      // Authoritative payload without this key — do not invent from a page slice.
      const fallbackKey = metricAggregateKey(name, metric);
      if (fallbackKey && typeof serverAggs[fallbackKey] === "number") {
        n = serverAggs[fallbackKey]!;
      } else if (metric.op === "count" && serverTotal != null) {
        n = serverTotal;
      } else {
        // Still compute client-side only when aggregate key is absent; prefer 0
        // over a truncated page for countWhere when totalCount says there are more.
        n = evaluateMetric(records, metric, today);
      }
    } else {
      n = evaluateMetric(records, metric, today);
    }
    values[name] = formatCountDisplay(n, records.length, queryLimit, authoritative);
  }

  const totalRaw =
    serverTotal != null
      ? serverTotal
      : typeof values.total === "string"
        ? Number(String(values.total).replace(/\+$/, ""))
        : records.length;
  if (totalRaw === 0 && typeof s.emptyText === "string" && s.emptyText.trim()) {
    return s.emptyText;
  }

  return s.template.replace(/\{([A-Za-z][A-Za-z0-9_]*)\}/g, (_, key: string) =>
    key in values ? values[key]! : "",
  );
}

/** Single metric for `stat` components bound to a rowsKey. */
export function evaluateRecordMetric(
  rowsValue: unknown,
  metric: unknown,
  now: Date = new Date(),
  queryLimit?: number,
): number | string | null {
  if (rowsValue === undefined || rowsValue === null) return null;
  if (!isMetric(metric)) return null;
  const serverAggs = authoritativeAggregates(rowsValue);
  const serverTotal = authoritativeTotalCount(rowsValue);
  if (metric.op === "count" && serverTotal != null) {
    return serverTotal;
  }
  // Stat components may name aggregates via a dedicated aggregates key matching
  // common patterns (done/remaining/overdue) when the query requested them.
  if (serverAggs) {
    if (metric.op === "countWhere" && "equals" in metric && typeof metric.equals === "string") {
      for (const [k, v] of Object.entries(serverAggs)) {
        if (k === metric.equals || k === metric.field) return v;
      }
    }
  }
  const records = unwrapRecords(rowsValue).slice(0, 500);
  const n = evaluateMetric(records, metric, localTodayIsoDate(now));
  const limit = parseQueryLimit(queryLimit);
  const authoritative = serverTotal != null || serverAggs != null;
  if (!authoritative && limit && records.length >= limit) {
    return formatCountDisplay(n, records.length, limit, false);
  }
  return n;
}

/** Milliseconds until next local midnight — for overdue refresh scheduling. */
export function msUntilLocalMidnight(now: Date = new Date()): number {
  const next = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
  return Math.max(1_000, next.getTime() - now.getTime());
}
