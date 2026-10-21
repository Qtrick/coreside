import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ToolComponent } from "@/types/tool";
import { ToolRuntimeProvider, type ToolRuntimeContextValue } from "./context";
import { DataTableNode } from "./nodes";

function runtime(
  state: Record<string, unknown>,
  setValue: (key: string, value: unknown) => void = vi.fn(),
): ToolRuntimeContextValue {
  return {
    toolId: "tool-test",
    state,
    runActions: vi.fn(),
    setValue,
    setValueOptimistic: vi.fn(),
    getValue: <T = unknown>(key: string, fallback?: T) =>
      (state[key] as T | undefined) ?? (fallback as T),
  };
}

function tableComponent(overrides: Partial<ToolComponent> = {}): ToolComponent {
  return {
    id: "dt-1",
    type: "dataTable",
    props: {
      title: "Tasks",
      searchable: false,
      columns: [
        { id: "title", label: "Title" },
        { id: "status", label: "Status" },
      ],
      valueKey: "rows",
      selectionKey: "selectedTaskId",
    },
    ...overrides,
  };
}

describe("DataTableNode keyboard Select button", () => {
  it("renders a Select button per row when selectionKey is wired", () => {
    const state = {
      rows: [
        { _id: "r1", title: "Write notes", status: "todo" },
        { _id: "r2", title: "Review", status: "done" },
      ],
      selectedTaskId: null,
    };
    render(
      <ToolRuntimeProvider value={runtime(state)}>
        <DataTableNode component={tableComponent()} renderChild={() => null} />
      </ToolRuntimeProvider>,
    );

    expect(screen.getByRole("button", { name: "Select Write notes" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Select Review" })).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "Select" })).toBeInTheDocument();
  });

  it("marks the selected row Selected and updates selection on activate", () => {
    const state: Record<string, unknown> = {
      rows: [
        { _id: "r1", title: "Write notes", status: "todo" },
        { _id: "r2", title: "Review", status: "done" },
      ],
      selectedTaskId: "r1",
    };
    const setValue = vi.fn((key: string, value: unknown) => {
      state[key] = value;
    });
    render(
      <ToolRuntimeProvider value={runtime(state, setValue)}>
        <DataTableNode component={tableComponent()} renderChild={() => null} />
      </ToolRuntimeProvider>,
    );

    const selected = screen.getByRole("button", {
      name: "Selected Write notes",
    });
    expect(selected).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Select Review" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );

    fireEvent.click(screen.getByRole("button", { name: "Select Review" }));
    expect(setValue).toHaveBeenCalledWith("selectedTaskId", "r2");
  });

  it("surfaces refresh errors above existing rows and does not spin forever when status is ok without totalCount", () => {
    const withError = {
      studyResult: {
        records: [{ _id: "r1", title: "Write notes", status: "todo" }],
        totalCount: 1,
        hasMore: false,
        offset: 0,
        limit: 25,
      },
      studyResult__status: "error",
      studyResult__error: "Network failed",
      studyOffset: 0,
    };
    const { rerender } = render(
      <ToolRuntimeProvider value={runtime(withError)}>
        <DataTableNode
          component={tableComponent({
            props: {
              title: "Tasks",
              searchable: false,
              columns: [{ id: "title", label: "Title" }],
              valueKey: "studyResult",
              offsetKey: "studyOffset",
              selectionKey: "selectedTaskId",
            },
          })}
          renderChild={() => null}
        />
      </ToolRuntimeProvider>,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("Network failed");
    expect(screen.getByText("Write notes")).toBeInTheDocument();

    const okMissingTotal = {
      studyResult: {
        records: [{ _id: "r1", title: "Write notes", status: "todo" }],
      },
      studyResult__status: "ok",
      studyOffset: 0,
    };
    rerender(
      <ToolRuntimeProvider value={runtime(okMissingTotal)}>
        <DataTableNode
          component={tableComponent({
            props: {
              title: "Tasks",
              searchable: false,
              columns: [{ id: "title", label: "Title" }],
              valueKey: "studyResult",
              offsetKey: "studyOffset",
            },
          })}
          renderChild={() => null}
        />
      </ToolRuntimeProvider>,
    );
    expect(screen.queryByText("Loading…")).not.toBeInTheDocument();
    expect(screen.getByText("Write notes")).toBeInTheDocument();
  });

  it("omits Select controls when selectionKey is absent", () => {
    const state = {
      rows: [{ _id: "r1", title: "Write notes", status: "todo" }],
    };
    render(
      <ToolRuntimeProvider value={runtime(state)}>
        <DataTableNode
          component={tableComponent({
            props: {
              title: "Tasks",
              searchable: false,
              columns: [{ id: "title", label: "Title" }],
              valueKey: "rows",
            },
          })}
          renderChild={() => null}
        />
      </ToolRuntimeProvider>,
    );

    expect(screen.queryByRole("button", { name: /Select/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("columnheader", { name: "Select" })).not.toBeInTheDocument();
  });
});
