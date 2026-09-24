import type { ComponentType, CSSProperties, ReactNode } from "react";
import { useEffect, useMemo, useState } from "react";
import type { ActionDefinition, ToolComponent } from "@/types/tool";
import { useToolRuntime } from "./context";
import { resolveMediaAssetUrl } from "@/lib/media";
import { api } from "@/lib/tauri";
import type { MediaAsset } from "@/types/media";

export type RenderChild = (component: ToolComponent) => ReactNode;

export type ToolNodeProps = {
  component: ToolComponent;
  renderChild: RenderChild;
};

function asString(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function asNumber(value: unknown, fallback = 0): number {
  const n = Number(value);
  return Number.isFinite(n) ? n : fallback;
}

/** Logical presentation size for generated scenes — reject nonfinite/negative/huge. */
function asBoundedSceneSize(value: unknown, fallback: number, max = 2048): number {
  const n = asNumber(value, fallback);
  if (!(n > 0) || !Number.isFinite(n)) return fallback;
  return Math.min(n, max);
}

function asBoolean(value: unknown, fallback = false): boolean {
  return typeof value === "boolean" ? value : fallback;
}

/**
 * Derives explicit state binding key for component.
 * P0 Security: Never falls back to component.id. Unbound components return null
 * and maintain local ephemeral React state.
 */
export function stateKeyFor(component: ToolComponent, ...propKeys: string[]): string | null {
  const allowValueKey = propKeys.length === 0 || propKeys.includes("valueKey");
  if (allowValueKey && component.valueKey && typeof component.valueKey === "string" && component.valueKey.trim()) {
    return component.valueKey.trim();
  }
  for (const propKey of propKeys) {
    const value = component.props?.[propKey];
    if (typeof value === "string" && value.trim()) return value.trim();
  }
  return null;
}

/**
 * Hook to manage bound state vs local ephemeral state.
 * If key is non-null, delegates to useToolRuntime.
 * If key is null, uses local React state.
 */
function useBoundState<T>(
  key: string | null,
  defaultValue: T,
  optimistic = false,
): [T, (val: T) => void] {
  const { getValue, setValue, setValueOptimistic } = useToolRuntime();
  const [localVal, setLocalVal] = useState<T>(defaultValue);

  if (key) {
    const remoteVal = getValue<T>(key, defaultValue);
    const setRemoteVal = (val: T) => {
      if (optimistic) {
        setValueOptimistic(key, val);
      } else {
        setValue(key, val);
      }
    };
    return [remoteVal, setRemoteVal];
  }

  return [localVal, setLocalVal];
}

export function ContainerNode({ component, renderChild }: ToolNodeProps) {
  return (
    <div className="tr-container" data-component-id={component.id}>
      {component.children?.map((child) => (
        <div key={child.id}>{renderChild(child)}</div>
      ))}
    </div>
  );
}

export function RowNode({ component, renderChild }: ToolNodeProps) {
  return (
    <div className="tr-row" data-component-id={component.id}>
      {component.children?.map((child) => (
        <div key={child.id}>{renderChild(child)}</div>
      ))}
    </div>
  );
}

export function ColumnNode({ component, renderChild }: ToolNodeProps) {
  return (
    <div className="tr-column" data-component-id={component.id}>
      {component.children?.map((child) => (
        <div key={child.id}>{renderChild(child)}</div>
      ))}
    </div>
  );
}

export function CardNode({ component, renderChild }: ToolNodeProps) {
  const title = asString(component.props?.title);
  return (
    <section className="tr-card" data-component-id={component.id} aria-label={title || undefined}>
      {title ? <h3 className="tr-heading">{title}</h3> : null}
      {component.children?.map((child) => (
        <div key={child.id}>{renderChild(child)}</div>
      ))}
    </section>
  );
}

export function TabsNode({ component, renderChild }: ToolNodeProps) {
  const { getValue, setValueOptimistic } = useToolRuntime();
  const tabs = Array.isArray(component.props?.tabs)
    ? (component.props?.tabs as Array<{ id: string; label: string }>)
    : (component.children ?? []).map((child) => ({
        id: child.id,
        label: asString(child.props?.label, child.id),
      }));
  const explicitKey = stateKeyFor(component, "valueKey");
  const [localActive, setLocalActive] = useState(tabs[0]?.id || "");
  const active = explicitKey
    ? asString(getValue(explicitKey), "") || tabs[0]?.id || ""
    : localActive || tabs[0]?.id || "";

  const handleSelectTab = (tabId: string) => {
    if (explicitKey) {
      setValueOptimistic(explicitKey, tabId);
    } else {
      setLocalActive(tabId);
    }
  };

  return (
    <div className="tr-tabs" data-component-id={component.id}>
      <div
        className="tr-tablist"
        role="tablist"
        aria-label={asString(component.props?.label, "Tabs")}
      >
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            className="btn btn-secondary"
            role="tab"
            aria-selected={active === tab.id}
            onClick={() => handleSelectTab(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>
      <div role="tabpanel">
        {component.children
          ?.filter(
            (child) =>
              child.id === active || asString(child.props?.tabId) === active,
          )
          .map((child) => (
            <div key={child.id}>{renderChild(child)}</div>
          ))}
      </div>
    </div>
  );
}

export function DividerNode({ component }: ToolNodeProps) {
  return <hr className="tr-divider" data-component-id={component.id} />;
}

export function SpacerNode({ component }: ToolNodeProps) {
  const size = asNumber(component.props?.size, 16);
  return (
    <div
      className="tr-spacer"
      data-component-id={component.id}
      style={{ height: size } satisfies CSSProperties}
      aria-hidden
    />
  );
}

export function HeadingNode({ component }: ToolNodeProps) {
  const level = Math.min(6, Math.max(1, asNumber(component.props?.level, 2)));
  const Tag = `h${level}` as unknown as ComponentType<{
    className?: string;
    children?: ReactNode;
    "data-component-id"?: string;
  }>;
  return (
    <Tag className="tr-heading" data-component-id={component.id}>
      {asString(component.props?.text, asString(component.props?.children))}
    </Tag>
  );
}

export function TextNode({ component }: ToolNodeProps) {
  return (
    <p className="tr-text" data-component-id={component.id}>
      {asString(component.props?.text, asString(component.props?.children))}
    </p>
  );
}

export function BadgeNode({ component }: ToolNodeProps) {
  const variant = asString(component.props?.variant, "default");
  return (
    <span
      className={`tr-badge${variant === "accent" ? " accent" : ""}`}
      data-component-id={component.id}
    >
      {asString(component.props?.text, asString(component.props?.label))}
    </span>
  );
}

export function ImageNode({ component }: ToolNodeProps) {
  const [resolvedSrc, setResolvedSrc] = useState<string | null>(null);
  const rawSrc = asString(component.props?.src);
  const assetId = asString(component.props?.mediaAssetId ?? component.props?.assetId);
  const alt = asString(component.props?.alt, "");

  useEffect(() => {
    let cancelled = false;
    if (assetId) {
      resolveMediaAssetUrl(assetId)
        .then((url) => {
          if (!cancelled) setResolvedSrc(url);
        })
        .catch(() => {
          if (!cancelled) setResolvedSrc(null);
        });
    } else if (rawSrc) {
      const trimmed = rawSrc.trim();
      // Security boundary: Block arbitrary remote http(s)://, file://, javascript:,
      // and root/relative filesystem paths from generated surfaces.
      // Remote assets must be imported into the Media Library first.
      const isSafeProtocol =
        trimmed.startsWith("asset://") ||
        trimmed.startsWith("tauri://") ||
        trimmed.startsWith("https://asset.localhost/") ||
        trimmed.startsWith("blob:") ||
        /^data:image\/(png|jpeg|jpg|webp|gif|bmp);base64,[a-z0-9+/=]+$/i.test(trimmed);

      if (isSafeProtocol) {
        setResolvedSrc(trimmed);
      } else {
        setResolvedSrc(null);
      }
    } else {
      setResolvedSrc(null);
    }
    return () => {
      cancelled = true;
    };
  }, [assetId, rawSrc]);

  if (!assetId && !rawSrc) {
    return (
      <div className="tr-fallback" data-component-id={component.id}>
        Image source missing
      </div>
    );
  }

  if (rawSrc && !assetId && !resolvedSrc) {
    return (
      <div className="tr-fallback tr-security-blocked" data-component-id={component.id}>
        Image source restricted. Import images into the Media Library first.
      </div>
    );
  }

  if (!resolvedSrc) {
    return (
      <div className="tr-image tr-image-loading" data-component-id={component.id}>
        <span className="muted">Loading image…</span>
      </div>
    );
  }

  return (
    <div className="tr-image" data-component-id={component.id}>
      <img src={resolvedSrc} alt={alt} />
    </div>
  );
}

export function EmptyStateNode({ component }: ToolNodeProps) {
  return (
    <div className="empty-state" data-component-id={component.id}>
      <h3>{asString(component.props?.title, "Nothing here yet")}</h3>
      <p>{asString(component.props?.description, asString(component.props?.text))}</p>
    </div>
  );
}

export function TextInputNode({ component }: ToolNodeProps) {
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const [value, setVal] = useBoundState(key, asString(component.props?.defaultValue, ""));
  const label = asString(component.props?.label, "Text");
  return (
    <div className="tr-field" data-component-id={component.id}>
      <label htmlFor={component.id}>{label}</label>
      <input
        id={component.id}
        type="text"
        value={value}
        placeholder={asString(component.props?.placeholder)}
        onChange={(e) => setVal(e.target.value)}
      />
    </div>
  );
}

export function TextAreaNode({ component }: ToolNodeProps) {
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const [value, setVal] = useBoundState(key, asString(component.props?.defaultValue, ""));
  const label = asString(component.props?.label, "Notes");
  return (
    <div className="tr-field" data-component-id={component.id}>
      <label htmlFor={component.id}>{label}</label>
      <textarea
        id={component.id}
        rows={asNumber(component.props?.rows, 4)}
        value={value}
        placeholder={asString(component.props?.placeholder)}
        onChange={(e) => setVal(e.target.value)}
      />
    </div>
  );
}

export function NumberInputNode({ component }: ToolNodeProps) {
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const [value, setVal] = useBoundState(key, asNumber(component.props?.defaultValue, 0));
  const label = asString(component.props?.label, "Number");
  return (
    <div className="tr-field" data-component-id={component.id}>
      <label htmlFor={component.id}>{label}</label>
      <input
        id={component.id}
        type="number"
        value={value}
        min={component.props?.min as number | undefined}
        max={component.props?.max as number | undefined}
        step={component.props?.step as number | undefined}
        onChange={(e) => setVal(Number(e.target.value))}
      />
    </div>
  );
}

export function SelectNode({ component }: ToolNodeProps) {
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const [value, setVal] = useBoundState(key, asString(component.props?.defaultValue, ""));
  const label = asString(component.props?.label, "Select");
  const options = Array.isArray(component.props?.options)
    ? (component.props.options as Array<{ value: string; label: string } | string>)
    : [];
  return (
    <div className="tr-field" data-component-id={component.id}>
      <label htmlFor={component.id}>{label}</label>
      <select
        id={component.id}
        value={value}
        onChange={(e) => setVal(e.target.value)}
      >
        {options.map((option) => {
          const optValue = typeof option === "string" ? option : option.value;
          const optLabel = typeof option === "string" ? option : option.label;
          return (
            <option key={optValue} value={optValue}>
              {optLabel}
            </option>
          );
        })}
      </select>
    </div>
  );
}

export function CheckboxNode({ component }: ToolNodeProps) {
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const [checked, setChecked] = useBoundState(
    key,
    asBoolean(component.props?.defaultValue, false),
    true,
  );
  const label = asString(component.props?.label, "Checkbox");
  return (
    <label className="tr-checkbox" data-component-id={component.id}>
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => setChecked(e.target.checked)}
      />
      <span>{label}</span>
    </label>
  );
}

export function DateInputNode({ component }: ToolNodeProps) {
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const [value, setVal] = useBoundState(key, asString(component.props?.defaultValue, ""));
  const label = asString(component.props?.label, "Date");
  return (
    <div className="tr-field" data-component-id={component.id}>
      <label htmlFor={component.id}>{label}</label>
      <input
        id={component.id}
        type="date"
        value={value}
        onChange={(e) => setVal(e.target.value)}
      />
    </div>
  );
}

export function ListNode({ component }: ToolNodeProps) {
  const { getValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const rawData = key ? getValue(key) : undefined;
  let items: unknown[] = [];
  if (Array.isArray(rawData)) {
    items = rawData;
  } else if (rawData && typeof rawData === "object") {
    const obj = rawData as Record<string, unknown>;
    if (Array.isArray(obj.records)) items = obj.records;
    else if (Array.isArray(obj.items)) items = obj.items;
  } else if (Array.isArray(component.props?.items)) {
    items = component.props.items as unknown[];
  }

  return (
    <ul className="tr-list" data-component-id={component.id}>
      {items.map((item, index) => (
        <li key={index}>
          {typeof item === "string"
            ? item
            : item && typeof item === "object" && "label" in item
              ? String((item as { label: unknown }).label)
              : JSON.stringify(item)}
        </li>
      ))}
    </ul>
  );
}

export function ChecklistNode({ component }: ToolNodeProps) {
  const { getValue, setValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const [localItems, setLocalItems] = useState<Array<{ id: string; label: string; checked?: boolean }>>(
    Array.isArray(component.props?.items)
      ? (component.props.items as Array<{ id: string; label: string; checked?: boolean }>)
      : [],
  );
  const items = key
    ? (Array.isArray(getValue(key))
        ? (getValue(key) as Array<{ id: string; label: string; checked?: boolean }>)
        : localItems)
    : localItems;

  const handleToggle = (index: number, checked: boolean) => {
    const next = items.map((entry, i) =>
      i === index ? { ...entry, checked } : entry,
    );
    if (key) {
      setValue(key, next);
    } else {
      setLocalItems(next);
    }
  };

  return (
    <ul className="tr-checklist" data-component-id={component.id}>
      {items.map((item, index) => (
        <li key={item.id ?? index}>
          <label className="tr-checkbox">
            <input
              type="checkbox"
              checked={Boolean(item.checked)}
              onChange={(e) => handleToggle(index, e.target.checked)}
            />
            <span>{item.label}</span>
          </label>
        </li>
      ))}
    </ul>
  );
}

export function TableNode({ component }: ToolNodeProps) {
  const { getValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey", "dataKey", "rowsKey");
  const stateRows = key ? getValue<unknown[]>(key) : undefined;
  let rawRows: unknown[] = [];
  if (Array.isArray(stateRows)) {
    rawRows = stateRows;
  } else if (stateRows && typeof stateRows === "object") {
    const obj = stateRows as Record<string, unknown>;
    if (Array.isArray(obj.records)) rawRows = obj.records;
    else if (Array.isArray(obj.rows)) rawRows = obj.rows;
    else if (Array.isArray(obj.items)) rawRows = obj.items;
    else if (Array.isArray(obj.data)) rawRows = obj.data;
  } else if (Array.isArray(component.props?.rows)) {
    rawRows = component.props.rows as unknown[];
  }
  const rows = rawRows as Array<Record<string, unknown>>;
  const columns = Array.isArray(component.props?.columns)
    ? (component.props.columns as Array<{ key: string; label: string } | string>)
    : [];
  const normalized = columns.map((col) =>
    typeof col === "string" ? { key: col, label: col } : col,
  );

  return (
    <div className="tr-table-wrap" data-component-id={component.id}>
      <table className="tr-table">
        <thead>
          <tr>
            {normalized.map((col) => (
              <th key={col.key} scope="col">
                {col.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, index) => (
            <tr key={index}>
              {normalized.map((col) => (
                <td key={col.key}>{String(row[col.key] ?? "")}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function CounterNode({ component }: ToolNodeProps) {
  const { getValue } = useToolRuntime();
  const key = stateKeyFor(component, "stateKey", "valueKey");
  const value = key
    ? asNumber(getValue(key, component.props?.defaultValue ?? 0))
    : asNumber(component.props?.defaultValue ?? 0);
  return (
    <div className="tr-counter" data-component-id={component.id}>
      <span className="muted">{asString(component.props?.label, "Count")}</span>
      <span className="tr-counter-value">{value}</span>
    </div>
  );
}

export function ClockNode({ component }: ToolNodeProps) {
  const { getValue, setValue } = useToolRuntime();
  const hour24Key = stateKeyFor(component, "hour24Key");
  const [localHour24, setLocalHour24] = useState(asBoolean(component.props?.hour24, false));
  const [showSeconds, setShowSeconds] = useState(asBoolean(component.props?.showSeconds ?? true, true));
  const [showDate] = useState(asBoolean(component.props?.showDate ?? true, true));
  const hour24 = hour24Key
    ? asBoolean(getValue(hour24Key, localHour24), localHour24)
    : localHour24;
  const title = asString(component.props?.title, "Clock");
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 250);
    return () => window.clearInterval(id);
  }, []);

  let hours = now.getHours();
  let suffix = "";
  if (!hour24) {
    suffix = hours >= 12 ? " PM" : " AM";
    hours = hours % 12;
    if (hours === 0) hours = 12;
  }
  const pad = (n: number) => String(n).padStart(2, "0");
  let time = `${pad(hours)}:${pad(now.getMinutes())}`;
  if (showSeconds) time += `:${pad(now.getSeconds())}`;
  time += suffix;

  const toggleHour24 = () => {
    if (hour24Key) {
      setValue(hour24Key, !hour24);
    } else {
      setLocalHour24(!hour24);
    }
  };

  return (
    <div className="tr-clock" data-component-id={component.id}>
      {title ? <div className="tr-clock-title">{title}</div> : null}
      <div className="tr-clock-time" aria-live="polite">
        {time}
      </div>
      {showDate ? (
        <div className="tr-clock-date">
          {now.toLocaleDateString(undefined, {
            weekday: "long",
            year: "numeric",
            month: "long",
            day: "numeric",
          })}
        </div>
      ) : null}
      <div className="tr-clock-controls button-row">
        <button
          type="button"
          className="btn btn-secondary"
          aria-pressed={hour24}
          onClick={toggleHour24}
        >
          {hour24 ? "24-hour" : "12-hour"}
        </button>
        <button
          type="button"
          className="btn btn-secondary"
          aria-pressed={showSeconds}
          onClick={() => setShowSeconds(!showSeconds)}
        >
          {showSeconds ? "Hide seconds" : "Show seconds"}
        </button>
      </div>
    </div>
  );
}

export function ProgressNode({ component }: ToolNodeProps) {
  const { getValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const value = asNumber(key ? getValue(key, component.props?.value ?? 0) : (component.props?.value ?? 0));
  const max = Math.max(1, asNumber(component.props?.max, 100));
  const pct = Math.max(0, Math.min(100, (value / max) * 100));
  return (
    <div className="tr-progress" data-component-id={component.id}>
      <div className="muted">{asString(component.props?.label, "Progress")}</div>
      <div
        className="tr-progress-track"
        role="progressbar"
        aria-valuenow={value}
        aria-valuemin={0}
        aria-valuemax={max}
      >
        <div className="tr-progress-fill" style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

export function StatNode({ component }: ToolNodeProps) {
  const { getValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const value = key ? getValue(key, component.props?.value ?? "—") : (component.props?.value ?? "—");
  return (
    <div className="tr-stat" data-component-id={component.id}>
      <span className="muted">{asString(component.props?.label, "Stat")}</span>
      <span className="tr-stat-value">{String(value)}</span>
    </div>
  );
}

/**
 * P0 Security: Authoritative action resolution.
 * Only component.actions is valid. Legacy props.action and props.actions are strictly ignored.
 */
function resolveButtonActions(component: ToolComponent): ActionDefinition[] {
  if (Array.isArray(component.actions) && component.actions.length > 0) {
    return component.actions;
  }
  return [];
}

export function ButtonNode({ component }: ToolNodeProps) {
  const { runActions } = useToolRuntime();
  const variant = asString(component.props?.variant, "secondary");
  const className =
    variant === "primary"
      ? "btn btn-primary"
      : variant === "danger"
        ? "btn btn-danger"
        : "btn btn-secondary";
  return (
    <button
      type="button"
      className={className}
      data-component-id={component.id}
      onClick={() => runActions(resolveButtonActions(component), component.id)}
    >
      {asString(component.props?.label, "Button")}
    </button>
  );
}

export function ButtonGroupNode({ component, renderChild }: ToolNodeProps) {
  return (
    <div className="tr-button-group" data-component-id={component.id} role="group">
      {component.children?.map((child) => (
        <div key={child.id}>{renderChild(child)}</div>
      ))}
    </div>
  );
}

type QuizQuestion = {
  id: string;
  prompt: string;
  choices: string[];
  correctIndex: number;
  explanation?: string;
};

type QuizState = {
  currentIndex: number;
  score: number;
  selected: number | null;
  answered: boolean;
  finished: boolean;
};

const defaultQuizState: QuizState = {
  currentIndex: 0,
  score: 0,
  selected: null,
  answered: false,
  finished: false,
};

export function QuizNode({ component }: ToolNodeProps) {
  const { getValue, setValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const [localQuizState, setLocalQuizState] = useState<QuizState>(defaultQuizState);
  const state: QuizState = key
    ? { ...defaultQuizState, ...(getValue<Partial<QuizState>>(key, {}) ?? {}) }
    : localQuizState;

  const updateQuizState = (nextState: QuizState) => {
    if (key) {
      setValue(key, nextState);
    } else {
      setLocalQuizState(nextState);
    }
  };

  const rawQuestions = Array.isArray(component.props?.questions)
    ? (component.props.questions as Array<Record<string, unknown>>)
    : [];
  const questions: QuizQuestion[] = rawQuestions.map((q, index) => {
    const choices = Array.isArray(q.choices)
      ? (q.choices as string[])
      : Array.isArray(q.options)
        ? (q.options as string[])
        : [];
    let correctIndex =
      typeof q.correctIndex === "number" ? q.correctIndex : -1;
    if (correctIndex < 0 && typeof q.answer === "string") {
      correctIndex = choices.findIndex((c) => c === q.answer);
    }
    if (correctIndex < 0 && typeof q.correct === "string") {
      correctIndex = choices.findIndex((c) => c === q.correct);
    }
    return {
      id: typeof q.id === "string" ? q.id : `q-${index + 1}`,
      prompt: typeof q.prompt === "string" ? q.prompt : `Question ${index + 1}`,
      choices,
      correctIndex: Math.max(0, correctIndex),
      explanation:
        typeof q.explanation === "string" ? q.explanation : undefined,
    };
  });
  const question = questions[state.currentIndex];

  if (!questions.length) {
    return (
      <div className="tr-fallback" data-component-id={component.id}>
        Quiz has no questions.
      </div>
    );
  }

  if (state.finished || !question) {
    return (
      <div className="tr-quiz" data-component-id={component.id}>
        <h3 className="tr-heading">Quiz complete</h3>
        <p className="tr-text">
          Score: {state.score} / {questions.length}
        </p>
        <button
          type="button"
          className="btn btn-secondary"
          onClick={() => updateQuizState({ ...defaultQuizState })}
        >
          Reset
        </button>
      </div>
    );
  }

  const selectChoice = (index: number) => {
    if (state.answered) return;
    const correct = index === question.correctIndex;
    updateQuizState({
      ...state,
      selected: index,
      answered: true,
      score: correct ? state.score + 1 : state.score,
    });
  };

  const next = () => {
    const nextIndex = state.currentIndex + 1;
    if (nextIndex >= questions.length) {
      updateQuizState({ ...state, finished: true });
      return;
    }
    updateQuizState({
      ...state,
      currentIndex: nextIndex,
      selected: null,
      answered: false,
    });
  };

  return (
    <div className="tr-quiz" data-component-id={component.id}>
      <div className="muted">
        Question {state.currentIndex + 1} of {questions.length} · Score {state.score}
      </div>
      <h3 className="tr-heading">{question.prompt}</h3>
      <div className="tr-quiz-choices" role="group" aria-label="Answer choices">
        {question.choices.map((choice, index) => {
          let extra = "";
          if (state.answered && index === question.correctIndex) extra = " correct";
          if (state.answered && state.selected === index && index !== question.correctIndex) {
            extra = " incorrect";
          }
          return (
            <button
              key={`${question.id}-${index}`}
              type="button"
              className={`btn btn-secondary${extra}`}
              onClick={() => selectChoice(index)}
              disabled={state.answered}
            >
              {choice}
            </button>
          );
        })}
      </div>
      {state.answered && question.explanation ? (
        <p className="tr-text muted">{question.explanation}</p>
      ) : null}
      <div className="button-row">
        <button
          type="button"
          className="btn btn-primary"
          onClick={next}
          disabled={!state.answered}
        >
          {state.currentIndex + 1 >= questions.length ? "Finish" : "Next"}
        </button>
        <button
          type="button"
          className="btn btn-ghost"
          onClick={() => updateQuizState({ ...defaultQuizState })}
        >
          Reset
        </button>
      </div>
    </div>
  );
}

/** Runtime V2 — rich form + capability-pack nodes (trusted, no CDN). */

export function FormNode({ component, renderChild }: ToolNodeProps) {
  const { runActions } = useToolRuntime();
  const reusable = asBoolean(component.props?.reusable, true);
  const [submitted, setSubmitted] = useState(false);
  if (!reusable && submitted) {
    return (
      <div className="tr-form tr-form-done" data-component-id={component.id}>
        <p className="muted">Submitted</p>
      </div>
    );
  }
  return (
    <form
      className="tr-form"
      data-component-id={component.id}
      onSubmit={(e) => {
        e.preventDefault();
        const actions = (component.actions ?? []) as ActionDefinition[];
        runActions(actions);
        if (!reusable) setSubmitted(true);
      }}
    >
      {component.children?.map((child) => (
        <div key={child.id}>{renderChild(child)}</div>
      ))}
    </form>
  );
}

export function FieldGroupNode({ component, renderChild }: ToolNodeProps) {
  const legend = asString(component.props?.label, asString(component.props?.legend));
  return (
    <fieldset className="tr-field-group" data-component-id={component.id}>
      {legend ? <legend>{legend}</legend> : null}
      {component.children?.map((child) => (
        <div key={child.id}>{renderChild(child)}</div>
      ))}
    </fieldset>
  );
}

export function RadioGroupNode({ component }: ToolNodeProps) {
  const key = stateKeyFor(component, "valueKey");
  const [value, setVal] = useBoundState(key, asString(component.props?.defaultValue, ""));
  const options = Array.isArray(component.props?.options)
    ? (component.props?.options as Array<{ value: string; label: string }>)
    : [];
  return (
    <div className="tr-radio-group" role="radiogroup" aria-label={asString(component.props?.label, key ?? "Options")} data-component-id={component.id}>
      {options.map((opt) => (
        <label key={opt.value} className="tr-radio">
          <input
            type="radio"
            name={component.id}
            value={opt.value}
            checked={value === opt.value}
            onChange={() => setVal(opt.value)}
          />
          {opt.label}
        </label>
      ))}
    </div>
  );
}

export function SliderNode({ component }: ToolNodeProps) {
  const key = stateKeyFor(component, "valueKey");
  const min = asNumber(component.props?.min, 0);
  const max = asNumber(component.props?.max, 100);
  const step = asNumber(component.props?.step, 1);
  const [value, setVal] = useBoundState(key, asNumber(component.props?.defaultValue, min));
  return (
    <label className="tr-slider" data-component-id={component.id}>
      <span>{asString(component.props?.label, "Slider")}: {value}</span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={value}
        onChange={(e) => setVal(Number(e.target.value))}
      />
    </label>
  );
}

export function SwitchNode({ component }: ToolNodeProps) {
  const key = stateKeyFor(component, "valueKey");
  const [on, setOn] = useBoundState(key, asBoolean(component.props?.defaultValue, false), true);
  return (
    <label className="tr-switch" data-component-id={component.id}>
      <input
        type="checkbox"
        role="switch"
        checked={on}
        onChange={() => setOn(!on)}
      />
      {asString(component.props?.label, "Toggle")}
    </label>
  );
}

export function ColorInputNode({ component }: ToolNodeProps) {
  const key = stateKeyFor(component, "valueKey");
  const [value, setVal] = useBoundState(key, asString(component.props?.defaultValue, "#2f8f63"));
  return (
    <label className="tr-color" data-component-id={component.id}>
      {asString(component.props?.label, "Color")}
      <input type="color" value={value} onChange={(e) => setVal(e.target.value)} />
    </label>
  );
}

export function TimeInputNode({ component }: ToolNodeProps) {
  const key = stateKeyFor(component, "valueKey");
  const [value, setVal] = useBoundState(key, asString(component.props?.defaultValue, ""));
  return (
    <label className="tr-time" data-component-id={component.id}>
      {asString(component.props?.label, "Time")}
      <input type="time" value={value} onChange={(e) => setVal(e.target.value)} />
    </label>
  );
}

export function DateTimeInputNode({ component }: ToolNodeProps) {
  const key = stateKeyFor(component, "valueKey");
  const [value, setVal] = useBoundState(key, asString(component.props?.defaultValue, ""));
  return (
    <label className="tr-datetime" data-component-id={component.id}>
      {asString(component.props?.label, "Date & time")}
      <input type="datetime-local" value={value} onChange={(e) => setVal(e.target.value)} />
    </label>
  );
}

export function SubmitButtonNode({ component }: ToolNodeProps) {
  return (
    <button type="submit" className="btn btn-primary" data-component-id={component.id}>
      {asString(component.props?.label, "Submit")}
    </button>
  );
}

export function ResetButtonNode({ component }: ToolNodeProps) {
  return (
    <button type="reset" className="btn btn-secondary" data-component-id={component.id}>
      {asString(component.props?.label, "Reset")}
    </button>
  );
}

export function ValidationMessageNode({ component }: ToolNodeProps) {
  const message = asString(component.props?.message);
  if (!message) return null;
  return (
    <p className="tr-validation" role="alert" data-component-id={component.id}>
      {message}
    </p>
  );
}

export function FilePickerNode({ component }: ToolNodeProps) {
  return (
    <p className="muted tr-file-picker" data-component-id={component.id}>
      File picker is limited to Media Library imports — use mediaPicker or import via chat.
      {asString(component.props?.label) ? ` (${asString(component.props?.label)})` : ""}
    </p>
  );
}

export function MediaPickerNode({ component }: ToolNodeProps) {
  const key = stateKeyFor(component, "valueKey");
  const [value, setVal] = useBoundState(key, asString(component.props?.defaultValue, ""));
  const [isOpen, setIsOpen] = useState(false);
  const [mediaAssets, setMediaAssets] = useState<MediaAsset[]>([]);
  const [loading, setLoading] = useState(false);
  const [search, setSearch] = useState("");
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    if (value) {
      resolveMediaAssetUrl(value)
        .then((url) => {
          if (!cancelled) setPreviewUrl(url);
        })
        .catch(() => {
          if (!cancelled) setPreviewUrl(null);
        });
    } else {
      setPreviewUrl(null);
    }
    return () => {
      cancelled = true;
    };
  }, [value]);

  const loadMedia = async () => {
    setLoading(true);
    try {
      const assets = await api.listMediaAssets(null, 50);
      setMediaAssets(assets);
    } catch {
      setMediaAssets([]);
    } finally {
      setLoading(false);
    }
  };

  const openPicker = () => {
    setIsOpen(true);
    void loadMedia();
  };

  const filteredAssets = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return mediaAssets;
    return mediaAssets.filter((a) => a.title.toLowerCase().includes(q));
  }, [mediaAssets, search]);

  return (
    <div className="tr-media-picker-container" data-component-id={component.id}>
      <span className="tr-field-label">{asString(component.props?.label, "Media Asset")}</span>
      <div className="tr-media-picker-controls button-row">
        {previewUrl ? (
          <div className="tr-media-preview-box">
            <img src={previewUrl} alt={value} className="tr-media-thumb" style={{ width: 48, height: 48, objectFit: "cover", borderRadius: 4 }} />
            <span className="muted" style={{ fontSize: "0.85em" }}>{value}</span>
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              onClick={() => setVal("")}
              title="Clear selection"
            >
              Clear
            </button>
          </div>
        ) : (
          <span className="muted">No media selected</span>
        )}
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={openPicker}
        >
          {value ? "Change Media…" : "Choose Media…"}
        </button>
      </div>

      {isOpen ? (
        <div
          className="tr-media-modal-backdrop"
          onClick={() => setIsOpen(false)}
          style={{
            position: "fixed",
            inset: 0,
            backgroundColor: "rgba(0, 0, 0, 0.6)",
            zIndex: 1000,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <div
            className="tr-media-modal"
            onClick={(e) => e.stopPropagation()}
            style={{
              background: "var(--color-bg-base, #1e2220)",
              border: "1px solid var(--color-border, #333)",
              borderRadius: 8,
              padding: 16,
              width: "90%",
              maxWidth: 480,
              maxHeight: "80vh",
              overflow: "auto",
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
              <h4 style={{ margin: 0 }}>Select Media Asset</h4>
              <button
                type="button"
                className="btn btn-ghost btn-sm"
                onClick={() => setIsOpen(false)}
              >
                ✕
              </button>
            </div>
            <input
              type="search"
              className="input input-sm"
              placeholder="Filter media by title…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              style={{ width: "100%", marginBottom: 12 }}
              autoFocus
            />
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(min(100%, 130px), 1fr))", gap: 8 }}>
              {loading ? (
                <p className="muted">Loading media assets…</p>
              ) : filteredAssets.length === 0 ? (
                <p className="muted">No media found in library</p>
              ) : (
                filteredAssets.map((asset) => (
                  <button
                    key={asset.id}
                    type="button"
                    className={`btn btn-secondary${value === asset.id ? " active" : ""}`}
                    style={{ display: "flex", flexDirection: "column", alignItems: "flex-start", padding: 8, height: "auto" }}
                    onClick={() => {
                      setVal(asset.id);
                      setIsOpen(false);
                    }}
                  >
                    <span style={{ fontWeight: 600, fontSize: "0.85em", wordBreak: "break-all" }}>{asset.title}</span>
                    <span className="muted" style={{ fontSize: "0.75em" }}>{asset.mimeType}</span>
                  </button>
                ))
              )}
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}

export function SvgSceneNode({ component, renderChild }: ToolNodeProps) {
  const width = asBoundedSceneSize(component.props?.width, 320);
  const height = asBoundedSceneSize(component.props?.height, 240);
  const viewBox = asString(component.props?.viewBox, `0 0 ${width} ${height}`);
  return (
    <svg
      className="tr-svg-scene"
      width={width}
      height={height}
      viewBox={viewBox}
      role="img"
      aria-label={asString(component.props?.ariaLabel, asString(component.props?.title, "SVG diagram"))}
      data-component-id={component.id}
    >
      {component.children?.map((child) => (
        <g key={child.id}>{renderChild(child)}</g>
      ))}
    </svg>
  );
}

const SAFE_SVG_ATTRS = new Set([
  "x", "y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "rx", "ry", "width", "height",
  "d", "points", "fill", "stroke", "strokeWidth", "stroke-width", "opacity",
  "fillOpacity", "strokeOpacity", "transform", "viewBox", "preserveAspectRatio",
  "fontSize", "font-size", "textAnchor", "text-anchor", "dominantBaseline",
  "clipPath", "clip-path", "offset", "stopColor", "stop-color", "stopOpacity",
]);

function svgProps(component: ToolComponent): Record<string, string | number> {
  const out: Record<string, string | number> = {};
  const props = component.props ?? {};
  for (const [k, v] of Object.entries(props)) {
    if (k.startsWith("on") || k === "dangerouslySetInnerHTML") continue;
    if (k === "href" || k === "xlinkHref" || k === "xlink:href") continue;
    if (!SAFE_SVG_ATTRS.has(k)) continue;
    if (typeof v === "string") {
      const lower = v.trim().toLowerCase();
      if (lower.startsWith("javascript:") || lower.startsWith("data:text/html")) continue;
      out[k] = v;
    } else if (typeof v === "number" && Number.isFinite(v)) {
      out[k] = v;
    }
  }
  return out;
}

export function SvgRectNode({ component }: ToolNodeProps) {
  return <rect data-component-id={component.id} {...svgProps(component)} />;
}
export function SvgCircleNode({ component }: ToolNodeProps) {
  return <circle data-component-id={component.id} {...svgProps(component)} />;
}
export function SvgEllipseNode({ component }: ToolNodeProps) {
  return <ellipse data-component-id={component.id} {...svgProps(component)} />;
}
export function SvgLineNode({ component }: ToolNodeProps) {
  return <line data-component-id={component.id} {...svgProps(component)} />;
}
export function SvgPathNode({ component }: ToolNodeProps) {
  return <path data-component-id={component.id} {...svgProps(component)} />;
}
export function SvgTextNode({ component }: ToolNodeProps) {
  return (
    <text data-component-id={component.id} {...svgProps(component)}>
      {asString(component.props?.text)}
    </text>
  );
}
export function SvgGroupNode({ component, renderChild }: ToolNodeProps) {
  return (
    <g data-component-id={component.id} {...svgProps(component)}>
      {component.children?.map((child) => (
        <g key={child.id}>{renderChild(child)}</g>
      ))}
    </g>
  );
}

function ChartNode({ component, kind }: { component: ToolComponent; kind: string }) {
  const { getValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "dataKey", "rowsKey");
  const stateData = key ? getValue(key) : undefined;
  let rawData: unknown[] = [];
  if (Array.isArray(stateData)) {
    rawData = stateData;
  } else if (stateData && typeof stateData === "object") {
    const obj = stateData as Record<string, unknown>;
    if (Array.isArray(obj.records)) rawData = obj.records;
    else if (Array.isArray(obj.points)) rawData = obj.points;
    else if (Array.isArray(obj.data)) rawData = obj.data;
  } else if (Array.isArray(component.props?.data)) {
    rawData = component.props.data as unknown[];
  } else if (Array.isArray(component.props?.points)) {
    rawData = component.props.points as unknown[];
  }

  const bounded = (rawData as Array<{ label?: string; value?: number; x?: number; y?: number }>).slice(0, 2000);
  const title = asString(component.props?.title, kind);
  const ariaLabel = asString(component.props?.ariaLabel, `${title} chart`);

  // Compute metrics
  const values = bounded.map((d) => (d.value != null ? Number(d.value) : Number(d.y ?? 0)));
  const maxVal = Math.max(1, ...values.map((v) => Math.abs(v)));
  const width = 360;
  const height = 200;
  const padL = 36;
  const padR = 16;
  const padT = 20;
  const padB = 30;
  const plotW = width - padL - padR;
  const plotH = height - padT - padB;

  const palette = ["#4f8df7", "#3dd68c", "#f7b955", "#e8657a", "#a27bf7", "#46c5e3"];

  const renderSvgContent = () => {
    if (bounded.length === 0) {
      return (
        <text x={width / 2} y={height / 2} textAnchor="middle" fill="currentColor" opacity={0.5} fontSize={13}>
          No data available
        </text>
      );
    }

    if (kind === "bar") {
      const barW = Math.max(8, Math.min(36, (plotW / bounded.length) * 0.7));
      const step = plotW / bounded.length;
      return (
        <g>
          <line x1={padL} y1={padT + plotH} x2={width - padR} y2={padT + plotH} stroke="currentColor" opacity={0.2} />
          {bounded.map((d, i) => {
            const v = values[i] ?? 0;
            const barH = (Math.abs(v) / maxVal) * plotH;
            const x = padL + i * step + (step - barW) / 2;
            const y = padT + plotH - barH;
            return (
              <g key={i}>
                <rect
                  x={x}
                  y={y}
                  width={barW}
                  height={Math.max(2, barH)}
                  rx={3}
                  fill={palette[i % palette.length]}
                  opacity={0.85}
                >
                  <title>{`${d.label ?? i}: ${v}`}</title>
                </rect>
                <text
                  x={x + barW / 2}
                  y={height - 10}
                  textAnchor="middle"
                  fontSize={10}
                  fill="currentColor"
                  opacity={0.65}
                >
                  {String(d.label ?? i).slice(0, 5)}
                </text>
              </g>
            );
          })}
        </g>
      );
    }

    if (kind === "line" || kind === "area") {
      const step = bounded.length > 1 ? plotW / (bounded.length - 1) : plotW;
      const pts = bounded.map((_, i) => {
        const v = values[i] ?? 0;
        const x = padL + i * step;
        const y = padT + plotH - (Math.abs(v) / maxVal) * plotH;
        return { x, y };
      });
      const pathD = pts.reduce((acc, p, i) => `${acc} ${i === 0 ? "M" : "L"} ${p.x.toFixed(1)} ${p.y.toFixed(1)}`, "");
      const areaD = pts.length > 0
        ? `${pathD} L ${pts[pts.length - 1]!.x.toFixed(1)} ${padT + plotH} L ${pts[0]!.x.toFixed(1)} ${padT + plotH} Z`
        : "";

      return (
        <g>
          <line x1={padL} y1={padT} x2={width - padR} y2={padT} stroke="currentColor" opacity={0.1} strokeDasharray="3 3" />
          <line x1={padL} y1={padT + plotH / 2} x2={width - padR} y2={padT + plotH / 2} stroke="currentColor" opacity={0.1} strokeDasharray="3 3" />
          <line x1={padL} y1={padT + plotH} x2={width - padR} y2={padT + plotH} stroke="currentColor" opacity={0.2} />

          {kind === "area" && areaD ? (
            <path d={areaD} fill="#4f8df7" opacity={0.2} />
          ) : null}
          <path d={pathD} fill="none" stroke="#4f8df7" strokeWidth={2.5} strokeLinecap="round" strokeLinejoin="round" />
          {pts.map((p, i) => (
            <g key={i}>
              <circle cx={p.x} cy={p.y} r={3.5} fill="#4f8df7" stroke="var(--color-bg-base, #1a1a1a)" strokeWidth={1.5}>
                <title>{`${bounded[i]?.label ?? i}: ${values[i]}`}</title>
              </circle>
              {bounded.length <= 12 ? (
                <text x={p.x} y={height - 10} textAnchor="middle" fontSize={10} fill="currentColor" opacity={0.65}>
                  {String(bounded[i]?.label ?? i).slice(0, 5)}
                </text>
              ) : null}
            </g>
          ))}
        </g>
      );
    }

    if (kind === "pie" || kind === "donut") {
      const cx = width / 2;
      const cy = height / 2;
      const r = Math.min(plotW, plotH) / 2 - 4;
      const innerR = kind === "donut" ? r * 0.55 : 0;
      const total = values.reduce((sum, v) => sum + Math.max(0, v), 0) || 1;

      let currentAngle = -Math.PI / 2;
      const slices = bounded.map((d, i) => {
        const v = Math.max(0, values[i] ?? 0);
        const fraction = v / total;
        const angle = fraction * Math.PI * 2;
        const start = currentAngle;
        const end = currentAngle + angle;
        currentAngle = end;

        const x1 = cx + r * Math.cos(start);
        const y1 = cy + r * Math.sin(start);
        const x2 = cx + r * Math.cos(end);
        const y2 = cy + r * Math.sin(end);
        const largeArc = angle > Math.PI ? 1 : 0;

        let dPath = "";
        if (innerR > 0) {
          const ix1 = cx + innerR * Math.cos(end);
          const iy1 = cy + innerR * Math.sin(end);
          const ix2 = cx + innerR * Math.cos(start);
          const iy2 = cy + innerR * Math.sin(start);
          dPath = `M ${x1} ${y1} A ${r} ${r} 0 ${largeArc} 1 ${x2} ${y2} L ${ix1} ${iy1} A ${innerR} ${innerR} 0 ${largeArc} 0 ${ix2} ${iy2} Z`;
        } else {
          dPath = `M ${cx} ${cy} L ${x1} ${y1} A ${r} ${r} 0 ${largeArc} 1 ${x2} ${y2} Z`;
        }
        return { dPath, label: d.label ?? String(i), value: v, color: palette[i % palette.length] };
      });

      return (
        <g>
          {slices.map((s, i) => (
            <path key={i} d={s.dPath} fill={s.color} opacity={0.88} stroke="var(--color-bg-base, #1a1a1a)" strokeWidth={1.5}>
              <title>{`${s.label}: ${s.value}`}</title>
            </path>
          ))}
        </g>
      );
    }

    if (kind === "scatter") {
      const maxX = Math.max(1, ...bounded.map((d) => Math.abs(Number(d.x ?? 0))));
      return (
        <g>
          <line x1={padL} y1={padT + plotH} x2={width - padR} y2={padT + plotH} stroke="currentColor" opacity={0.2} />
          <line x1={padL} y1={padT} x2={padL} y2={padT + plotH} stroke="currentColor" opacity={0.2} />
          {bounded.map((d, i) => {
            const xv = Number(d.x ?? i);
            const yv = values[i] ?? 0;
            const cx = padL + (Math.abs(xv) / maxX) * plotW;
            const cy = padT + plotH - (Math.abs(yv) / maxVal) * plotH;
            return (
              <circle
                key={i}
                cx={cx}
                cy={cy}
                r={4.5}
                fill={palette[i % palette.length]}
                opacity={0.8}
              >
                <title>{`${d.label ?? i}: (${xv}, ${yv})`}</title>
              </circle>
            );
          })}
        </g>
      );
    }

    return null;
  };

  return (
    <figure className="tr-chart" data-component-id={component.id} data-chart={kind}>
      <figcaption className="tr-heading">{title}</figcaption>
      <div className="tr-chart-svg-wrap" role="img" aria-label={ariaLabel}>
        <svg
          viewBox={`0 0 ${width} ${height}`}
          width="100%"
          height="100%"
          preserveAspectRatio="xMidYMid meet"
          className="tr-chart-svg"
          style={{ maxHeight: 240 }}
        >
          {renderSvgContent()}
        </svg>
      </div>
      <table className="tr-chart-a11y">
        <caption>Accessible data</caption>
        <thead>
          <tr>
            <th scope="col">Label</th>
            <th scope="col">Value</th>
          </tr>
        </thead>
        <tbody>
          {bounded.map((d, i) => (
            <tr key={i}>
              <td>{d.label ?? String(i)}</td>
              <td>{values[i]}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </figure>
  );
}

export function ChartLineNode({ component }: ToolNodeProps) {
  return <ChartNode component={component} kind="line" />;
}
export function ChartBarNode({ component }: ToolNodeProps) {
  return <ChartNode component={component} kind="bar" />;
}
export function ChartPieNode({ component }: ToolNodeProps) {
  return <ChartNode component={component} kind="pie" />;
}
export function ChartDonutNode({ component }: ToolNodeProps) {
  return <ChartNode component={component} kind="donut" />;
}
export function ChartAreaNode({ component }: ToolNodeProps) {
  return <ChartNode component={component} kind="area" />;
}
export function ChartScatterNode({ component }: ToolNodeProps) {
  return <ChartNode component={component} kind="scatter" />;
}

export function CodeEditorNode({ component }: ToolNodeProps) {
  const { runActions } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey");
  const defaultVal = asString(component.props?.value, asString(component.props?.defaultValue, ""));
  const [value, setVal] = useBoundState(key, defaultVal);
  const readOnly = asBoolean(component.props?.readOnly);
  const language = asString(component.props?.language, "text");
  return (
    <div className="tr-code-editor" data-component-id={component.id}>
      <div className="muted">Code editor ({language}) — does not execute code</div>
      <textarea
        className="tr-code-area"
        value={value}
        readOnly={readOnly}
        spellCheck={false}
        aria-label={asString(component.props?.ariaLabel, "Code editor")}
        rows={asNumber(component.props?.rows, 12)}
        onChange={(e) => setVal(e.target.value.slice(0, 200_000))}
      />
      {component.actions?.length ? (
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => runActions(component.actions as ActionDefinition[])}
        >
          {asString(component.props?.submitLabel, "Submit to agent")}
        </button>
      ) : null}
    </div>
  );
}

export function MathInlineNode({ component }: ToolNodeProps) {
  const expr = asString(component.props?.expression, asString(component.props?.tex));
  return (
    <span className="tr-math-inline" data-component-id={component.id} aria-label={asString(component.props?.ariaLabel, expr)}>
      <code>{expr}</code>
    </span>
  );
}

export function MathBlockNode({ component }: ToolNodeProps) {
  const expr = asString(component.props?.expression, asString(component.props?.tex));
  return (
    <div className="tr-math-block" data-component-id={component.id} role="math" aria-label={asString(component.props?.ariaLabel, expr)}>
      <pre><code>{expr}</code></pre>
    </div>
  );
}

export function CanvasSceneNode({ component }: ToolNodeProps) {
  const objects = Array.isArray(component.props?.objects)
    ? (component.props?.objects as Array<Record<string, unknown>>).slice(0, 200)
    : [];
  const width = asBoundedSceneSize(component.props?.width, 320);
  const height = asBoundedSceneSize(component.props?.height, 240);
  return (
    <div
      className="tr-canvas-scene"
      data-component-id={component.id}
      style={{ width, height, position: "relative", border: "1px solid var(--border)" }}
      role="img"
      aria-label={asString(component.props?.ariaLabel, "Canvas scene")}
    >
      {objects.map((obj, i) => {
        const type = asString(obj.type, "rect");
        const style: CSSProperties = {
          position: "absolute",
          left: asNumber(obj.x),
          top: asNumber(obj.y),
          width: asNumber(obj.width, 40),
          height: asNumber(obj.height, 40),
          background: asString(obj.fill, "var(--accent-primary)"),
          borderRadius: type === "circle" ? "50%" : undefined,
        };
        return <div key={i} style={style} title={asString(obj.label)} />;
      })}
    </div>
  );
}

export function AudioPlayerNode({ component }: ToolNodeProps) {
  const [resolvedSrc, setResolvedSrc] = useState<string | null>(null);
  const assetId = asString(component.props?.mediaAssetId ?? component.props?.assetId);
  const rawSrc = asString(component.props?.src);

  useEffect(() => {
    let cancelled = false;
    if (assetId) {
      resolveMediaAssetUrl(assetId)
        .then((url) => {
          if (!cancelled) setResolvedSrc(url);
        })
        .catch(() => {
          if (!cancelled) setResolvedSrc(null);
        });
    } else if (rawSrc) {
      const trimmed = rawSrc.trim();
      const isSafeProtocol =
        trimmed.startsWith("asset://") ||
        trimmed.startsWith("tauri://") ||
        trimmed.startsWith("https://asset.localhost/") ||
        trimmed.startsWith("blob:") ||
        /^data:audio\/(mpeg|mp3|wav|ogg|aac|webm);base64,[a-z0-9+/=]+$/i.test(trimmed);

      if (isSafeProtocol) {
        setResolvedSrc(trimmed);
      } else {
        setResolvedSrc(null);
      }
    } else {
      setResolvedSrc(null);
    }
    return () => {
      cancelled = true;
    };
  }, [assetId, rawSrc]);

  if (!resolvedSrc) {
    return (
      <p className="muted tr-security-blocked" data-component-id={component.id}>
        Audio requires an approved Media Library source.
      </p>
    );
  }
  return (
    <audio
      className="tr-audio"
      data-component-id={component.id}
      controls
      preload="metadata"
      src={resolvedSrc}
      aria-label={asString(component.props?.ariaLabel, "Audio player")}
    />
  );
}

export function DataTableNode({ component }: ToolNodeProps) {
  const { getValue, setValue, runActions } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey", "dataKey", "rowsKey");
  const selectionKey = stateKeyFor(component, "selectionKey");
  const searchKey = stateKeyFor(component, "searchKey");
  const selectedId = selectionKey ? asString(getValue(selectionKey)) : null;

  const stateRows = key ? getValue(key) : undefined;
  let rawRows: unknown[] = [];
  if (Array.isArray(stateRows)) {
    rawRows = stateRows;
  } else if (stateRows && typeof stateRows === "object") {
    const obj = stateRows as Record<string, unknown>;
    if (Array.isArray(obj.records)) rawRows = obj.records;
    else if (Array.isArray(obj.rows)) rawRows = obj.rows;
    else if (Array.isArray(obj.items)) rawRows = obj.items;
    else if (Array.isArray(obj.data)) rawRows = obj.data;
  } else if (Array.isArray(component.props?.rows)) {
    rawRows = component.props.rows as unknown[];
  }
  const rows = (rawRows as Array<Record<string, unknown>>).slice(0, 500);

  const columns = useMemo(() => {
    if (Array.isArray(component.props?.columns) && component.props.columns.length > 0) {
      return (component.props.columns as Array<Record<string, unknown> | string>).map((c) => {
        if (typeof c === "string") return { id: c, label: c };
        const id = asString(c.id ?? c.key ?? c.accessor, "");
        const label = asString(c.label ?? c.header, id ? id.charAt(0).toUpperCase() + id.slice(1) : "");
        return { id, label };
      });
    }
    if (rows.length > 0 && rows[0] && typeof rows[0] === "object") {
      return Object.keys(rows[0])
        .filter((k) => !k.startsWith("_"))
        .map((k) => ({ id: k, label: k.charAt(0).toUpperCase() + k.slice(1) }));
    }
    return [];
  }, [component.props?.columns, rows]);

  const pageSizeProp = asNumber(component.props?.pageSize, 10);
  const pageSize = pageSizeProp > 0 && pageSizeProp <= 100 ? pageSizeProp : 10;

  const [filterText, setFilterText] = useState("");
  const [sortColumn, setSortColumn] = useState<string | null>(null);
  const [sortDir, setSortDir] = useState<"asc" | "desc" | null>(null);
  const [currentPage, setCurrentPage] = useState(1);

  const externalSearch = searchKey ? asString(getValue(searchKey)).trim().toLowerCase() : "";
  const filtersFromState = (component.props?.filtersFromState ?? component.props?.filters) as
    | Record<string, string>
    | undefined;

  // Filter
  const filteredRows = useMemo(() => {
    let result = rows;

    // Apply external filtersFromState (e.g. { priority: "filterPriority", status: "filterStatus" })
    if (filtersFromState && typeof filtersFromState === "object") {
      for (const [colField, stateVar] of Object.entries(filtersFromState)) {
        if (typeof stateVar === "string" && stateVar) {
          const rawFilterVal = getValue(stateVar);
          if (rawFilterVal != null && rawFilterVal !== "" && rawFilterVal !== "all" && rawFilterVal !== "All") {
            const expected = String(rawFilterVal).toLowerCase();
            result = result.filter((row) => {
              const actual = row[colField];
              return actual != null && String(actual).toLowerCase() === expected;
            });
          }
        }
      }
    }

    // Apply external searchKey query if present
    if (externalSearch) {
      result = result.filter((row) =>
        columns.some((col) => {
          const val = row[col.id];
          return val != null && String(val).toLowerCase().includes(externalSearch);
        })
      );
    }

    // Apply internal toolbar search input
    const q = filterText.trim().toLowerCase();
    if (q) {
      result = result.filter((row) =>
        columns.some((col) => {
          const val = row[col.id];
          return val != null && String(val).toLowerCase().includes(q);
        })
      );
    }

    return result;
  }, [rows, columns, filterText, externalSearch, filtersFromState, getValue]);

  // Sort
  const sortedRows = useMemo(() => {
    if (!sortColumn || !sortDir) return filteredRows;
    const sorted = [...filteredRows];
    sorted.sort((a, b) => {
      const valA = a[sortColumn];
      const valB = b[sortColumn];
      if (valA == null && valB == null) return 0;
      if (valA == null) return sortDir === "asc" ? 1 : -1;
      if (valB == null) return sortDir === "asc" ? -1 : 1;

      if (typeof valA === "number" && typeof valB === "number") {
        return sortDir === "asc" ? valA - valB : valB - valA;
      }
      const strA = String(valA).toLowerCase();
      const strB = String(valB).toLowerCase();
      return sortDir === "asc" ? strA.localeCompare(strB) : strB.localeCompare(strA);
    });
    return sorted;
  }, [filteredRows, sortColumn, sortDir]);

  // Pagination
  const totalPages = Math.max(1, Math.ceil(sortedRows.length / pageSize));
  const safePage = Math.min(Math.max(1, currentPage), totalPages);
  const pagedRows = useMemo(() => {
    const start = (safePage - 1) * pageSize;
    return sortedRows.slice(start, start + pageSize);
  }, [sortedRows, safePage, pageSize]);

  const handleSort = (colId: string) => {
    if (sortColumn !== colId) {
      setSortColumn(colId);
      setSortDir("asc");
    } else if (sortDir === "asc") {
      setSortDir("desc");
    } else {
      setSortColumn(null);
      setSortDir(null);
    }
  };

  const handleRowClick = (row: Record<string, unknown>, i: number) => {
    if (selectionKey) {
      const id = asString(row._id ?? row.id ?? i);
      setValue(selectionKey, id);
    }
    if (Array.isArray(component.actions) && component.actions.length > 0) {
      runActions(component.actions, component.id);
    }
  };

  const title = asString(component.props?.title, "Data table");
  const enableSearch = component.props?.searchable !== false;

  return (
    <div className="tr-data-table" data-component-id={component.id} role="region" aria-label={title}>
      {enableSearch || rows.length > 5 ? (
        <div className="tr-data-table-toolbar">
          <input
            type="search"
            className="tr-data-table-search"
            placeholder="Search records…"
            value={filterText}
            onChange={(e) => {
              setFilterText(e.target.value);
              setCurrentPage(1);
            }}
            aria-label="Filter records"
          />
          <span className="tr-data-table-summary">
            {sortedRows.length} {sortedRows.length === 1 ? "record" : "records"}
            {filterText ? ` (filtered from ${rows.length})` : ""}
          </span>
        </div>
      ) : null}

      <div className="tr-data-table-scroll">
        <table>
          <caption>{title}</caption>
          <thead>
            <tr>
              {columns.map((c) => {
                const isSorted = sortColumn === c.id;
                const ariaSort = isSorted ? (sortDir === "asc" ? "ascending" : "descending") : "none";
                return (
                  <th key={c.id} scope="col" aria-sort={ariaSort}>
                    <button
                      type="button"
                      onClick={() => handleSort(c.id)}
                      title={`Sort by ${c.label}`}
                    >
                      {c.label}
                      <span aria-hidden="true">
                        {isSorted ? (sortDir === "asc" ? " ▲" : " ▼") : " ⇅"}
                      </span>
                    </button>
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {pagedRows.length > 0 ? (
              pagedRows.map((row, i) => {
                const rowId = asString(row._id ?? row.id ?? i);
                const isSelected = selectedId === rowId;
                return (
                  <tr
                    key={row._id ? String(row._id) : i}
                    className={isSelected ? "tr-row-selected" : undefined}
                    aria-selected={selectionKey ? isSelected : undefined}
                    onClick={() => handleRowClick(row, i)}
                    style={selectionKey ? { cursor: "pointer" } : undefined}
                  >
                    {columns.map((c) => (
                      <td key={c.id}>{String(row[c.id] ?? "")}</td>
                    ))}
                  </tr>
                );
              })
            ) : (
              <tr>
                <td colSpan={columns.length || 1} className="tr-data-table-empty">
                  {filterText ? "No matching records found." : "No data available."}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {totalPages > 1 ? (
        <div className="tr-data-table-pagination">
          <span>
            Page {safePage} of {totalPages}
          </span>
          <div className="tr-data-table-pagination-controls">
            <button
              type="button"
              className="tr-data-table-pagination-btn"
              disabled={safePage <= 1}
              onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
            >
              Previous
            </button>
            <button
              type="button"
              className="tr-data-table-pagination-btn"
              disabled={safePage >= totalPages}
              onClick={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
            >
              Next
            </button>
          </div>
        </div>
      ) : null}
    </div>
  );
}

/** Compatibility fallback for legacy tools that still include dictationButton. */
export function DictationButtonNode({ component }: ToolNodeProps) {
  return (
    <div
      className="tr-fallback tr-unsupported-control"
      data-component-id={component.id}
      role="alert"
      aria-live="polite"
    >
      <strong>Unsupported control</strong>
      <p>
        Dictation is not available in this version of Coreside (
        <code>{component.type}</code>).
      </p>
    </div>
  );
}

export function UnsupportedNode({ component }: ToolNodeProps) {
  return (
    <div
      className="tr-fallback tr-unknown-component"
      data-component-id={component.id}
      role="alert"
      aria-live="polite"
    >
      <strong>Protected placeholder</strong>
      <p>
        This component type is not available in Coreside yet:{" "}
        <code>{component.type}</code>
      </p>
    </div>
  );
}
