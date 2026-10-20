import { describe, expect, it } from "vitest";
import {
  evaluateRecordMetric,
  formatRecordSummary,
  localTodayIsoDate,
  unwrapRecords,
} from "./record-summary";

describe("record-summary", () => {
  const records = [
    { title: "A", status: "todo", dueDate: "2020-01-01", priority: "high" },
    { title: "B", status: "done", dueDate: "2099-01-01", priority: "low" },
    { title: "C", status: "todo", dueDate: "2099-06-01", priority: "medium" },
  ];

  it("unwraps query payloads", () => {
    expect(unwrapRecords({ records, count: 3 })).toHaveLength(3);
    expect(unwrapRecords(records)).toHaveLength(3);
    expect(unwrapRecords(null)).toHaveLength(0);
  });

  it("formats declarative study-task style summaries", () => {
    const text = formatRecordSummary(
      { records },
      {
        template:
          "{total} study tasks · {remaining} remaining · {done} completed · {overdue} overdue · {high} high priority",
        metrics: {
          total: { op: "count" },
          done: { op: "countWhere", field: "status", equals: "done" },
          remaining: { op: "countWhere", field: "status", notEquals: "done" },
          overdue: {
            op: "countOverdue",
            field: "dueDate",
            statusField: "status",
            excludeStatus: "done",
          },
          high: { op: "countWhere", field: "priority", equals: "high" },
        },
        emptyText: "No study tasks yet.",
      },
      new Date("2024-06-15T12:00:00"),
    );
    expect(text).toBe(
      "3 study tasks · 2 remaining · 1 completed · 1 overdue · 1 high priority",
    );
  });

  it("uses emptyText when there are no records", () => {
    const text = formatRecordSummary(
      { records: [] },
      {
        template: "{total} study tasks",
        metrics: { total: { op: "count" } },
        emptyText: "No study tasks yet. Open Study Planner to add one.",
      },
    );
    expect(text).toContain("No study tasks yet");
  });

  it("rejects forged field names in metrics", () => {
    expect(
      evaluateRecordMetric({ records }, {
        op: "countWhere",
        field: "status;drop",
        equals: "done",
      }),
    ).toBe(0);
    expect(
      evaluateRecordMetric({ records }, {
        op: "countWhere",
        field: "_id",
        equals: "x",
      }),
    ).toBe(0);
  });

  it("compares overdue dates as date-only ISO strings", () => {
    const today = localTodayIsoDate(new Date("2024-06-15T23:00:00"));
    expect(today).toBe("2024-06-15");
    const overdue = evaluateRecordMetric(
      {
        records: [
          { status: "todo", dueDate: "2024-06-14" },
          { status: "todo", dueDate: "2024-06-15" },
          { status: "done", dueDate: "2024-01-01" },
        ],
      },
      {
        op: "countOverdue",
        field: "dueDate",
        statusField: "status",
        excludeStatus: "done",
      },
      new Date("2024-06-15T12:00:00"),
    );
    expect(overdue).toBe(1);
  });

  it("returns null for malformed summary specs", () => {
    expect(formatRecordSummary({ records }, null)).toBeNull();
    expect(formatRecordSummary({ records }, { template: "" })).toBeNull();
  });

  it("treats missing hydration as not-ready rather than empty", () => {
    const spec = {
      template: "{total} study tasks",
      metrics: { total: { op: "count" as const } },
      emptyText: "No study tasks yet.",
    };
    expect(formatRecordSummary(undefined, spec)).toBeNull();
    expect(formatRecordSummary(null, spec)).toBeNull();
    expect(evaluateRecordMetric(undefined, { op: "count" })).toBeNull();
    expect(evaluateRecordMetric(null, { op: "count" })).toBeNull();
  });

  it("unwraps rows and items aliases like DataTable payloads", () => {
    expect(unwrapRecords({ rows: records })).toHaveLength(3);
    expect(unwrapRecords({ items: records })).toHaveLength(3);
  });

  it("evaluates countWhere notEquals and rejects unknown metric ops", () => {
    expect(
      evaluateRecordMetric({ records }, {
        op: "countWhere",
        field: "status",
        notEquals: "done",
      }),
    ).toBe(2);
    expect(evaluateRecordMetric({ records }, { op: "sum", field: "priority" })).toBeNull();
  });

  it("leaves unknown template placeholders empty rather than injecting raw keys", () => {
    const text = formatRecordSummary(
      { records },
      {
        template: "{total} tasks · {forgedMetric} leftover",
        metrics: { total: { op: "count" } },
      },
    );
    expect(text).toBe("3 tasks ·  leftover");
  });
});
