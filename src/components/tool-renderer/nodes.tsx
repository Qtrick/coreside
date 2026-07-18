import type { ComponentType, CSSProperties, ReactNode } from "react";
import { useEffect, useState } from "react";
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
  const { getValue, setValue } = useToolRuntime();
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
            onClick={() => setValue(valueKey, tab.id)}
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
  const key = component.valueKey ?? component.id;
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
  const key = component.valueKey ?? component.id;
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
  const key = component.valueKey ?? component.id;
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
  const key = component.valueKey ?? component.id;
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
  const { getValue, setValue } = useToolRuntime();
  const key = component.valueKey ?? component.id;
  const label = asString(component.props?.label, "Checkbox");
  return (
    <label className="tr-checkbox" data-component-id={component.id}>
      <input
        type="checkbox"
        checked={asBoolean(getValue(key, component.props?.defaultValue ?? false))}
        onChange={(e) => setValue(key, e.target.checked)}
      />
      <span>{label}</span>
    </label>
  );
}

export function DateInputNode({ component }: ToolNodeProps) {
  const { getValue, setValue } = useToolRuntime();
  const key = component.valueKey ?? component.id;
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
  const key = component.valueKey ?? component.id;
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
  const key = component.valueKey ?? component.id;
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
  const columns = Array.isArray(component.props?.columns)
    ? (component.props.columns as Array<{ key: string; label: string } | string>)
    : [];
  const rows = Array.isArray(component.props?.rows)
    ? (component.props.rows as Array<Record<string, unknown>>)
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

export function UnsupportedNode({ component }: ToolNodeProps) {
  return (
    <div className="tr-fallback" data-component-id={component.id} role="alert">
      Unsupported component type: <strong>{component.type}</strong>
    </div>
  );
}
