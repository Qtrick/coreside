import { describe, expect, it } from "vitest";
import type { ToolDefinition } from "@/types/tool";
import {
  addSection,
  bindAction,
  bindState,
  filterStateByScope,
  findComponentWithRegion,
  findRegion,
  fromToolDefinition,
  getChildRegions,
  getRegionPath,
  moveComponent,
  preserveStateValues,
  removeSection,
  setStyleToken,
  toToolDefinition,
  updateRegionLayout,
  updateSection,
  validateAndRepair,
} from "./software-document";

describe("SoftwareDocument (UI IR)", () => {
  const sampleTool: ToolDefinition = {
    id: "tool-research-dash",
    name: "Research Dashboard",
    description: "Personal research dashboard",
    layout: { type: "single-column" },
    version: 1,
    components: [
      {
        id: "comp-search-input",
        type: "textInput",
        props: {
          section: "search-header",
          placeholder: "Search literature...",
        },
        valueKey: "query",
      },
      {
        id: "comp-search-btn",
        type: "button",
        props: {
          section: "search-header",
          label: "Search",
        },
        actions: [
          {
            type: "invokeRegisteredAction",
            actionName: "system.search",
            inputFromState: { q: "query" },
            resultKey: "searchResults",
          },
        ],
      },
      {
        id: "comp-results-table",
        type: "table",
        props: {
          section: "results-view",
          columns: ["title", "authors", "year"],
        },
        valueKey: "searchResults",
      },
    ],
  };

  it("converts ToolDefinition to SoftwareDocument and back preserving structure", () => {
    const doc = fromToolDefinition(sampleTool);
    expect(doc.id).toBe("tool-research-dash");
    expect(doc.title).toBe("Research Dashboard");
    expect(doc.sections.length).toBe(2);
    expect(doc.sections[0].id).toBe("search-header");
    expect(doc.sections[0].components.length).toBe(2);
    expect(doc.sections[1].id).toBe("results-view");
    expect(doc.sections[1].components.length).toBe(1);

    const backToTool = toToolDefinition(doc);
    expect(backToTool.id).toBe(sampleTool.id);
    expect(backToTool.name).toBe(sampleTool.name);
    expect(backToTool.components.length).toBe(3);
    expect(backToTool.components[0].id).toBe("comp-search-input");
    expect(backToTool.components[0].props?.section).toBe("search-header");
  });

  it("handles section addition, update, and removal", () => {
    let doc = fromToolDefinition(sampleTool);

    // Add section
    doc = addSection(doc, {
      id: "stats-sidebar",
      title: "Statistics",
      role: "sidebar",
      layout: "grid",
      components: [
        {
          id: "comp-stat-papers",
          type: "stat",
          props: { label: "Total Papers", value: "42" },
        },
      ],
    });
    expect(doc.sections.length).toBe(3);
    expect(doc.sections[2].id).toBe("stats-sidebar");

    // Update section
    doc = updateSection(doc, "stats-sidebar", {
      title: "Reading Metrics",
    });
    expect(doc.sections[2].title).toBe("Reading Metrics");

    // Remove section
    doc = removeSection(doc, "stats-sidebar");
    expect(doc.sections.length).toBe(2);
    expect(doc.sections.some((s) => s.id === "stats-sidebar")).toBe(false);
  });

  it("binds state and generates missing state contract", () => {
    let doc = fromToolDefinition(sampleTool);
    doc = bindState(doc, "comp-search-input", "customFilter", "all");

    expect(doc.stateContracts.some((s) => s.key === "customFilter")).toBe(true);
    const sec = doc.sections.find((s) => s.id === "search-header");
    const comp = sec?.components.find((c) => c.id === "comp-search-input");
    expect(comp?.valueKey).toBe("customFilter");
  });

  it("binds action and registers action contract", () => {
    let doc = fromToolDefinition(sampleTool);
    doc = bindAction(doc, "comp-search-btn", {
      type: "invokeRegisteredAction",
      actionName: "bookmarks.export",
      resultKey: "exportResult",
    });

    const actContract = doc.actionContracts.find((a) =>
      a.actionId.includes("bookmarks-export")
    );
    expect(actContract).toBeDefined();
    expect(actContract?.actionName).toBe("bookmarks.export");
    expect(actContract?.resultKey).toBe("exportResult");
  });

  it("applies safe style tokens to sections and components", () => {
    let doc = fromToolDefinition(sampleTool);

    // Section token
    doc = setStyleToken(doc, "search-header", "density", "compact");
    expect(doc.sections[0].metadata?.density).toBe("compact");

    // Component token
    doc = setStyleToken(doc, "comp-search-btn", "variant", "primary");
    const btn = doc.sections[0].components.find((c) => c.id === "comp-search-btn");
    expect(btn?.props?.variant).toBe("primary");
  });

  it("executes self-repair loop: deduplicates IDs, strips dangerous scripts and handlers, auto-declares state", () => {
    const messyDoc = fromToolDefinition({
      id: "tool-messy",
      name: "Messy Tool",
      description: "",
      components: [
        {
          id: "duplicate-id",
          type: "button",
          props: {
            label: "Click",
            onclick: "alert('pwned')",
            href: "javascript:void(0)",
          },
          valueKey: "orphanedKey",
        },
        {
          id: "duplicate-id",
          type: "text",
          props: {
            content: "Hello <script>malicious()</script>",
          },
        },
      ],
    });

    const { doc: repairedDoc, notes } = validateAndRepair(messyDoc);

    // Duplicate ID rectified
    const compIds = repairedDoc.sections[0].components.map((c) => c.id);
    expect(compIds[0]).not.toBe(compIds[1]);
    expect(notes.some((n) => n.kind === "deduplicated_component_id")).toBe(true);

    // Dangerous props stripped
    const comp0Props = repairedDoc.sections[0].components[0].props;
    expect(comp0Props?.onclick).toBeUndefined();
    expect(comp0Props?.href).toBeUndefined();
    expect(notes.some((n) => n.kind === "removed_event_handler")).toBe(true);
    expect(notes.some((n) => n.kind === "removed_script_injection")).toBe(true);

    // Auto-declared missing StateContract
    expect(repairedDoc.stateContracts.some((s) => s.key === "orphanedKey")).toBe(true);
    expect(notes.some((n) => n.kind === "auto_declared_state_contract")).toBe(true);
  });

  it("supports regional hierarchy, parent-child addressing, and moving components", () => {
    let doc = fromToolDefinition({
      id: "tool-regional-surface",
      name: "Regional Surface",
      description: "Regional surface description",
      version: 1,
      components: [],
    });

    doc = addSection(doc, {
      id: "header",
      role: "header",
      title: "App Header",
      components: [],
    });

    doc = addSection(doc, {
      id: "main",
      role: "content",
      title: "Main Content",
      components: [],
    });

    doc = addSection(doc, {
      id: "filters-slot",
      role: "sidebar",
      title: "Filters",
      parentRegionId: "main",
      slot: "left",
      components: [
        {
          id: "input-filter",
          type: "textInput",
          props: { placeholder: "Filter items..." },
        },
      ],
    });

    doc = addSection(doc, {
      id: "results-slot",
      role: "content",
      title: "Results",
      parentRegionId: "main",
      slot: "center",
      components: [],
    });

    // Check regional parent-child relationships
    const children = getChildRegions(doc, "main");
    expect(children.length).toBe(2);
    expect(children.map((c) => c.id)).toEqual(["filters-slot", "results-slot"]);

    // Check hierarchical region path
    const path = getRegionPath(doc, "filters-slot");
    expect(path).toBe("tool-regional-surface/main/filters-slot");

    // Component lookup with region
    const compWithRegion = findComponentWithRegion(doc, "input-filter");
    expect(compWithRegion).toBeDefined();
    expect(compWithRegion?.section.id).toBe("filters-slot");
    expect(compWithRegion?.component.id).toBe("input-filter");

    // Move component from filters-slot to results-slot
    doc = moveComponent(doc, "input-filter", "results-slot", 0);
    const moved = findComponentWithRegion(doc, "input-filter");
    expect(moved?.section.id).toBe("results-slot");
    expect(moved?.component.props?.section).toBe("results-slot");

    // Update region layout and responsive configuration
    doc = updateRegionLayout(doc, "results-slot", "grid", { cols: 2, breakpoint: 768 });
    const updatedSec = findRegion(doc, "results-slot");
    expect(updatedSec?.layout).toBe("grid");
    expect(updatedSec?.responsive).toEqual({ cols: 2, breakpoint: 768 });
  });

  it("manages explicit state scopes and preserves persistent/session state across updates", () => {
    const doc = fromToolDefinition({
      id: "tool-stateful-app",
      name: "Stateful App",
      description: "Stateful app description",
      version: 1,
      components: [],
    });

    doc.stateContracts.push({
      key: "search_query",
      type: "string",
      initialValue: "",
      scope: "persistent",
      preservationPolicy: "keep_on_patch",
    });

    doc.stateContracts.push({
      key: "selected_index",
      type: "number",
      initialValue: 0,
      scope: "session",
    });

    doc.stateContracts.push({
      key: "dropdown_open",
      type: "boolean",
      initialValue: false,
      scope: "ephemeral",
    });

    doc.stateContracts.push({
      key: "is_submitting",
      type: "boolean",
      initialValue: false,
      scope: "in_flight",
    });

    const activeState: Record<string, unknown> = {
      search_query: "Quantum biology",
      selected_index: 3,
      dropdown_open: true,
      is_submitting: true,
      untracked_field: "some user value",
    };

    // Filter by scope
    const persistent = filterStateByScope(doc, activeState, "persistent");
    expect(persistent).toEqual({ search_query: "Quantum biology" });

    const session = filterStateByScope(doc, activeState, "session");
    expect(session).toEqual({ selected_index: 3 });

    // Preserve states: Ephemeral & in_flight discarded; persistent, session & untracked preserved
    const preserved = preserveStateValues(doc, activeState);
    expect(preserved.search_query).toBe("Quantum biology");
    expect(preserved.selected_index).toBe(3);
    expect(preserved.untracked_field).toBe("some user value");
    expect(preserved.dropdown_open).toBeUndefined();
    expect(preserved.is_submitting).toBeUndefined();
  });
});

