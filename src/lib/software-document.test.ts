import { describe, expect, it } from "vitest";
import type { ToolDefinition } from "@/types/tool";
import {
  addSection,
  bindAction,
  bindState,
  fromToolDefinition,
  removeSection,
  setStyleToken,
  toToolDefinition,
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
});
