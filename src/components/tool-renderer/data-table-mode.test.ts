import { describe, expect, it } from "vitest";
import { isQueryBackedDataTable } from "./data-table-mode";

describe("isQueryBackedDataTable", () => {
  it("stays client-side when totalCount is present but no query keys are wired", () => {
    expect(
      isQueryBackedDataTable({
        totalCount: 150,
        offsetKey: null,
        sortFieldKey: null,
        sortDirKey: null,
        searchKey: null,
      }),
    ).toBe(false);
  });

  it("uses server mode when at least one query key is wired", () => {
    expect(
      isQueryBackedDataTable({
        totalCount: 150,
        offsetKey: "studyOffset",
      }),
    ).toBe(true);
    expect(
      isQueryBackedDataTable({
        totalCount: 10,
        searchKey: "studySearch",
      }),
    ).toBe(true);
    expect(
      isQueryBackedDataTable({
        totalCount: 10,
        sortFieldKey: "studySortField",
        sortDirKey: "studySortDir",
      }),
    ).toBe(true);
  });

  it("uses server mode before totalCount arrives so loading is not empty", () => {
    expect(
      isQueryBackedDataTable({
        totalCount: null,
        offsetKey: "studyOffset",
      }),
    ).toBe(true);
  });
});
