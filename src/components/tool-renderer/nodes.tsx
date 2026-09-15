import type { ComponentType, CSSProperties, ReactNode } from "react";
import { useEffect, useMemo, useState } from "react";
import type { ActionDefinition, ToolComponent } from "@/types/tool";
import { useToolRuntime } from "./context";

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

function stateKeyFor(component: ToolComponent, ...propKeys: string[]): string {
  if (component.valueKey) return component.valueKey;
  for (const propKey of propKeys) {
    const value = component.props?.[propKey];
    if (typeof value === "string" && value.trim()) return value;
  }
  return component.id;
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
  const valueKey = component.valueKey ?? `${component.id}:tab`;
  const active = asString(getValue(valueKey), "") || tabs[0]?.id || "";

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
            onClick={() => setValueOptimistic(valueKey, tab.id)}
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
  const src = asString(component.props?.src);
  const alt = asString(component.props?.alt, "");
  if (!src) {
    return (
      <div className="tr-fallback" data-component-id={component.id}>
        Image source missing
      </div>
    );
  }
  return (
    <div className="tr-image" data-component-id={component.id}>
      <img src={src} alt={alt} />
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
  const { getValue, setValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const label = asString(component.props?.label, "Text");
  return (
    <div className="tr-field" data-component-id={component.id}>
      <label htmlFor={component.id}>{label}</label>
      <input
        id={component.id}
        type="text"
        value={asString(getValue(key, asString(component.props?.defaultValue)))}
        placeholder={asString(component.props?.placeholder)}
        onChange={(e) => setValue(key, e.target.value)}
      />
    </div>
  );
}

export function TextAreaNode({ component }: ToolNodeProps) {
  const { getValue, setValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const label = asString(component.props?.label, "Notes");
  return (
    <div className="tr-field" data-component-id={component.id}>
      <label htmlFor={component.id}>{label}</label>
      <textarea
        id={component.id}
        rows={asNumber(component.props?.rows, 4)}
        value={asString(getValue(key, asString(component.props?.defaultValue)))}
        placeholder={asString(component.props?.placeholder)}
        onChange={(e) => setValue(key, e.target.value)}
      />
    </div>
  );
}

export function NumberInputNode({ component }: ToolNodeProps) {
  const { getValue, setValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const label = asString(component.props?.label, "Number");
  return (
    <div className="tr-field" data-component-id={component.id}>
      <label htmlFor={component.id}>{label}</label>
      <input
        id={component.id}
        type="number"
        value={asNumber(getValue(key, component.props?.defaultValue ?? 0))}
        min={component.props?.min as number | undefined}
        max={component.props?.max as number | undefined}
        step={component.props?.step as number | undefined}
        onChange={(e) => setValue(key, Number(e.target.value))}
      />
    </div>
  );
}

export function SelectNode({ component }: ToolNodeProps) {
  const { getValue, setValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const label = asString(component.props?.label, "Select");
  const options = Array.isArray(component.props?.options)
    ? (component.props.options as Array<{ value: string; label: string } | string>)
    : [];
  return (
    <div className="tr-field" data-component-id={component.id}>
      <label htmlFor={component.id}>{label}</label>
      <select
        id={component.id}
        value={asString(getValue(key, asString(component.props?.defaultValue)))}
        onChange={(e) => setValue(key, e.target.value)}
      >
        {options.map((option) => {
          const value = typeof option === "string" ? option : option.value;
          const optionLabel = typeof option === "string" ? option : option.label;
          return (
            <option key={value} value={value}>
              {optionLabel}
            </option>
          );
        })}
      </select>
    </div>
  );
}

export function CheckboxNode({ component }: ToolNodeProps) {
  const { getValue, setValueOptimistic } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const label = asString(component.props?.label, "Checkbox");
  return (
    <label className="tr-checkbox" data-component-id={component.id}>
      <input
        type="checkbox"
        checked={asBoolean(getValue(key, component.props?.defaultValue ?? false))}
        onChange={(e) => setValueOptimistic(key, e.target.checked)}
      />
      <span>{label}</span>
    </label>
  );
}

export function DateInputNode({ component }: ToolNodeProps) {
  const { getValue, setValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const label = asString(component.props?.label, "Date");
  return (
    <div className="tr-field" data-component-id={component.id}>
      <label htmlFor={component.id}>{label}</label>
      <input
        id={component.id}
        type="date"
        value={asString(getValue(key, asString(component.props?.defaultValue)))}
        onChange={(e) => setValue(key, e.target.value)}
      />
    </div>
  );
}

export function ListNode({ component }: ToolNodeProps) {
  const { getValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const items = Array.isArray(getValue(key))
    ? (getValue(key) as unknown[])
    : Array.isArray(component.props?.items)
      ? (component.props.items as unknown[])
      : [];
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
  const items = Array.isArray(getValue(key))
    ? (getValue(key) as Array<{ id: string; label: string; checked?: boolean }>)
    : Array.isArray(component.props?.items)
      ? (component.props.items as Array<{ id: string; label: string; checked?: boolean }>)
      : [];

  return (
    <ul className="tr-checklist" data-component-id={component.id}>
      {items.map((item, index) => (
        <li key={item.id ?? index}>
          <label className="tr-checkbox">
            <input
              type="checkbox"
              checked={Boolean(item.checked)}
              onChange={(e) => {
                const next = items.map((entry, i) =>
                  i === index ? { ...entry, checked: e.target.checked } : entry,
                );
                setValue(key, next);
              }}
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
  const stateRows = getValue<unknown[]>(key);
  const rawRows = Array.isArray(stateRows)
    ? stateRows
    : Array.isArray(component.props?.rows)
      ? (component.props.rows as unknown[])
      : [];
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
  const value = asNumber(getValue(key, component.props?.defaultValue ?? 0));
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
  const secondsKey = `${component.id}:showSeconds`;
  const dateKey = `${component.id}:showDate`;
  const hour24 = asBoolean(
    getValue(hour24Key, component.props?.hour24 ?? false),
    asBoolean(component.props?.hour24, false),
  );
  const showSeconds = asBoolean(
    getValue(secondsKey, component.props?.showSeconds ?? true),
    true,
  );
  const showDate = asBoolean(
    getValue(dateKey, component.props?.showDate ?? true),
    true,
  );
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
          onClick={() => setValue(hour24Key, !hour24)}
        >
          {hour24 ? "24-hour" : "12-hour"}
        </button>
        <button
          type="button"
          className="btn btn-secondary"
          aria-pressed={showSeconds}
          onClick={() => setValue(secondsKey, !showSeconds)}
        >
          Seconds
        </button>
        <button
          type="button"
          className="btn btn-secondary"
          aria-pressed={showDate}
          onClick={() => setValue(dateKey, !showDate)}
        >
          Date
        </button>
      </div>
    </div>
  );
}

export function ProgressNode({ component }: ToolNodeProps) {
  const { getValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey");
  const value = asNumber(getValue(key, component.props?.value ?? 0));
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
  const value = getValue(key, component.props?.value ?? "—");
  return (
    <div className="tr-stat" data-component-id={component.id}>
      <span className="muted">{asString(component.props?.label, "Stat")}</span>
      <span className="tr-stat-value">{String(value)}</span>
    </div>
  );
}

function resolveButtonActions(component: ToolComponent): ActionDefinition[] {
  if (component.actions && component.actions.length > 0) {
    return component.actions;
  }
  const props = component.props ?? {};
  if (Array.isArray(props.actions)) {
    return props.actions as ActionDefinition[];
  }
  const type = props.action;
  const target = props.target;
  if (typeof type === "string" && typeof target === "string") {
    const actionType = type === "set" ? "reset" : type;
    return [
      {
        type: actionType,
        target,
        ...(props.amount != null ? { amount: Number(props.amount) } : {}),
        ...(props.value !== undefined ? { value: props.value } : {}),
      } as ActionDefinition,
    ];
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
  const state = {
    ...defaultQuizState,
    ...(getValue<Partial<QuizState>>(key, {}) ?? {}),
  };
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
          onClick={() => setValue(key, { ...defaultQuizState })}
        >
          Reset
        </button>
      </div>
    );
  }

  const selectChoice = (index: number) => {
    if (state.answered) return;
    const correct = index === question.correctIndex;
    setValue(key, {
      ...state,
      selected: index,
      answered: true,
      score: correct ? state.score + 1 : state.score,
    });
  };

  const next = () => {
    const nextIndex = state.currentIndex + 1;
    if (nextIndex >= questions.length) {
      setValue(key, { ...state, finished: true });
      return;
    }
    setValue(key, {
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
          onClick={() => setValue(key, { ...defaultQuizState })}
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
  const { getValue, setValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey");
  const options = Array.isArray(component.props?.options)
    ? (component.props?.options as Array<{ value: string; label: string }>)
    : [];
  const value = asString(getValue(key), asString(component.props?.defaultValue));
  return (
    <div className="tr-radio-group" role="radiogroup" aria-label={asString(component.props?.label, key)} data-component-id={component.id}>
      {options.map((opt) => (
        <label key={opt.value} className="tr-radio">
          <input
            type="radio"
            name={key}
            value={opt.value}
            checked={value === opt.value}
            onChange={() => setValue(key, opt.value)}
          />
          {opt.label}
        </label>
      ))}
    </div>
  );
}

export function SliderNode({ component }: ToolNodeProps) {
  const { getValue, setValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey");
  const min = asNumber(component.props?.min, 0);
  const max = asNumber(component.props?.max, 100);
  const step = asNumber(component.props?.step, 1);
  const value = asNumber(getValue(key), asNumber(component.props?.defaultValue, min));
  return (
    <label className="tr-slider" data-component-id={component.id}>
      <span>{asString(component.props?.label, key)}: {value}</span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={value}
        onChange={(e) => setValue(key, Number(e.target.value))}
      />
    </label>
  );
}

export function SwitchNode({ component }: ToolNodeProps) {
  const { getValue, setValueOptimistic } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey");
  const on = asBoolean(getValue(key), asBoolean(component.props?.defaultValue));
  return (
    <label className="tr-switch" data-component-id={component.id}>
      <input
        type="checkbox"
        role="switch"
        checked={on}
        onChange={() => setValueOptimistic(key, !on)}
      />
      {asString(component.props?.label, key)}
    </label>
  );
}

export function ColorInputNode({ component }: ToolNodeProps) {
  const { getValue, setValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey");
  const value = asString(getValue(key), asString(component.props?.defaultValue, "#2f8f63"));
  return (
    <label className="tr-color" data-component-id={component.id}>
      {asString(component.props?.label, "Color")}
      <input type="color" value={value} onChange={(e) => setValue(key, e.target.value)} />
    </label>
  );
}

export function TimeInputNode({ component }: ToolNodeProps) {
  const { getValue, setValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey");
  const value = asString(getValue(key), asString(component.props?.defaultValue, ""));
  return (
    <label className="tr-time" data-component-id={component.id}>
      {asString(component.props?.label, "Time")}
      <input type="time" value={value} onChange={(e) => setValue(key, e.target.value)} />
    </label>
  );
}

export function DateTimeInputNode({ component }: ToolNodeProps) {
  const { getValue, setValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey");
  const value = asString(getValue(key), asString(component.props?.defaultValue, ""));
  return (
    <label className="tr-datetime" data-component-id={component.id}>
      {asString(component.props?.label, "Date & time")}
      <input type="datetime-local" value={value} onChange={(e) => setValue(key, e.target.value)} />
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
  const { getValue, setValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey");
  const value = asString(getValue(key));
  return (
    <label className="tr-media-picker" data-component-id={component.id}>
      {asString(component.props?.label, "Media asset id")}
      <input
        type="text"
        value={value}
        placeholder="media-…"
        onChange={(e) => setValue(key, e.target.value)}
      />
    </label>
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
  const data = Array.isArray(component.props?.data)
    ? (component.props?.data as Array<{ label?: string; value: number; x?: number; y?: number }>)
    : [];
  const bounded = data.slice(0, 2000);
  const max = Math.max(1, ...bounded.map((d) => Math.abs(Number(d.value) || 0)));
  return (
    <figure className="tr-chart" data-component-id={component.id} data-chart={kind}>
      <figcaption className="tr-heading">{asString(component.props?.title, kind)}</figcaption>
      <div className="tr-chart-bars" role="img" aria-label={asString(component.props?.ariaLabel, "Chart")}>
        {bounded.map((d, i) => {
          const v = Number(d.value) || 0;
          const h = Math.round((Math.abs(v) / max) * 100);
          return (
            <div key={i} className="tr-chart-bar-wrap" title={`${d.label ?? i}: ${v}`}>
              <div className="tr-chart-bar" style={{ height: `${h}%` }} />
              <span className="muted">{d.label ?? String(i)}</span>
            </div>
          );
        })}
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
              <td>{d.label ?? i}</td>
              <td>{d.value}</td>
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
  const { getValue, setValue, runActions } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey");
  const value = asString(getValue(key), asString(component.props?.value, asString(component.props?.defaultValue)));
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
        onChange={(e) => setValue(key, e.target.value.slice(0, 200_000))}
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
  const src = asString(component.props?.src);
  if (!src.startsWith("asset:") && !src.startsWith("blob:") && !src.startsWith("/")) {
    return (
      <p className="muted" data-component-id={component.id}>
        Audio requires a local Media Library source.
      </p>
    );
  }
  return (
    <audio
      className="tr-audio"
      data-component-id={component.id}
      controls
      preload="metadata"
      src={src}
      aria-label={asString(component.props?.ariaLabel, "Audio player")}
    />
  );
}

export function DataTableNode({ component }: ToolNodeProps) {
  const { getValue } = useToolRuntime();
  const key = stateKeyFor(component, "valueKey", "stateKey", "dataKey", "rowsKey");
  const stateRows = getValue<unknown[]>(key);
  const rawRows = Array.isArray(stateRows)
    ? stateRows
    : Array.isArray(component.props?.rows)
      ? (component.props.rows as unknown[])
      : [];
  const rows = (rawRows as Array<Record<string, unknown>>).slice(0, 500);
  const columns = Array.isArray(component.props?.columns)
    ? (component.props?.columns as Array<{ id: string; label: string }>)
    : [];

  const pageSizeProp = asNumber(component.props?.pageSize, 10);
  const pageSize = pageSizeProp > 0 && pageSizeProp <= 100 ? pageSizeProp : 10;

  const [filterText, setFilterText] = useState("");
  const [sortColumn, setSortColumn] = useState<string | null>(null);
  const [sortDir, setSortDir] = useState<"asc" | "desc" | null>(null);
  const [currentPage, setCurrentPage] = useState(1);

  // Filter
  const filteredRows = useMemo(() => {
    const q = filterText.trim().toLowerCase();
    if (!q) return rows;
    return rows.filter((row) =>
      columns.some((col) => {
        const val = row[col.id];
        return val != null && String(val).toLowerCase().includes(q);
      })
    );
  }, [rows, columns, filterText]);

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
              pagedRows.map((row, i) => (
                <tr key={i}>
                  {columns.map((c) => (
                    <td key={c.id}>{String(row[c.id] ?? "")}</td>
                  ))}
                </tr>
              ))
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
