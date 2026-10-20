/**
 * Trusted declarative metrics over queried local_data records.
 *
 * No eval, no arbitrary expressions — only a fixed set of count operations
 * evaluated against already-hydrated query results in the renderer.
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

function isMetric(value: unknown): value is RecordMetric {
  if (!value || typeof value !== "object") return false;
  const op = (value as { op?: unknown }).op;
  return (
    op === "count" ||
    op === "countWhere" ||
    op === "countOverdue"
  );
}

/**
 * Format a declarative summary from queried records.
 * Unknown metrics / malformed specs fall back to emptyText or "".
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
  const today = localTodayIsoDate(now);
  const entries = Object.entries(s.metrics).slice(0, MAX_METRICS);
  const values: Record<string, string> = {};
  for (const [name, metric] of entries) {
    if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name) || !isMetric(metric)) continue;
    values[name] = String(evaluateMetric(records, metric, today));
  }

  const total =
    typeof values.total === "string"
      ? Number(values.total)
      : records.length;
  if (total === 0 && typeof s.emptyText === "string" && s.emptyText.trim()) {
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
): number | null {
  if (rowsValue === undefined || rowsValue === null) return null;
  if (!isMetric(metric)) return null;
  const records = unwrapRecords(rowsValue).slice(0, 500);
  return evaluateMetric(records, metric, localTodayIsoDate(now));
}
