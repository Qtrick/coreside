import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { ComponentTypeSchema, type ComponentType, type ToolComponent, type ToolDefinition } from "@/types/tool";
import { resolveComponent } from "@/components/tool-renderer/registry";
import { UnsupportedNode } from "@/components/tool-renderer/nodes";
import { ToolRenderer } from "@/components/tool-renderer/ToolRenderer";

function samplePropsForType(type: ComponentType): Record<string, unknown> {
  switch (type) {
    case "heading":
      return { text: "Gallery Heading", level: 2 };
    case "text":
      return { text: "Sample informative text" };
    case "badge":
      return { text: "Active", variant: "accent" };
    case "image":
      return { src: "asset://media/demo.png", alt: "Demo image" };
    case "emptyState":
      return { title: "No Items", description: "Get started by creating one" };
    case "divider":
    case "spacer":
      return {};
    case "textInput":
    case "textArea":
      return { label: "Sample Field", placeholder: "Type here...", valueKey: "field" };
    case "numberInput":
      return { label: "Count", min: 0, max: 100, valueKey: "count" };
    case "select":
      return { label: "Choose Option", options: [{ label: "A", value: "a" }], valueKey: "choice" };
    case "checkbox":
      return { label: "Enable feature", valueKey: "enabled" };
    case "radioGroup":
      return { label: "Priority", options: [{ label: "High", value: "high" }], valueKey: "priority" };
    case "slider":
      return { label: "Volume", min: 0, max: 100, valueKey: "volume" };
    case "switch":
      return { label: "Notifications", valueKey: "notify" };
    case "dateInput":
      return { label: "Date", valueKey: "date" };
    case "timeInput":
      return { label: "Time", valueKey: "time" };
    case "dateTimeInput":
      return { label: "Event Time", valueKey: "datetime" };
    case "colorInput":
      return { label: "Color", valueKey: "color" };
    case "list":
    case "checklist":
      return { stateKey: "items", placeholder: "New item" };
    case "table":
    case "dataTable":
      return {
        columns: [{ id: "col1", header: "Name", accessor: "name" }],
        rows: [{ id: "1", name: "Alpha" }],
        dataKey: "tableData",
      };
    case "counter":
      return { label: "Score", min: 0, max: 10, stateKey: "score" };
    case "progress":
      return { label: "Progress", max: 100, value: 50 };
    case "stat":
      return { label: "Revenue", value: "$4,500" };
    case "clock":
      return { label: "Clock", timezone: "UTC" };
    case "button":
      return { label: "Click Me", variant: "primary" };
    case "submitButton":
      return { label: "Submit Form" };
    case "resetButton":
      return { label: "Clear Form" };
    case "buttonGroup":
      return {};
    case "quiz":
      return {
        questions: [{ id: "q1", prompt: "2+2?", options: ["3", "4"], correctIndex: 1 }],
      };
    case "form":
    case "fieldGroup":
      return { title: "Form Group" };
    case "filePicker":
    case "mediaPicker":
      return { label: "Upload Asset", stateKey: "upload" };
    case "validationMessage":
      return { message: "Invalid input provided" };
    case "svgScene":
      return { width: 200, height: 100 };
    case "svgRect":
      return { x: 10, y: 10, width: 50, height: 50, fill: "#3b82f6" };
    case "svgCircle":
      return { cx: 50, cy: 50, r: 25, fill: "#10b981" };
    case "svgEllipse":
      return { cx: 50, cy: 50, rx: 30, ry: 20, fill: "#f59e0b" };
    case "svgLine":
      return { x1: 0, y1: 0, x2: 100, y2: 100, stroke: "#6b7280" };
    case "svgPath":
      return { d: "M10 10 H 90 V 90 H 10 Z", fill: "#ef4444" };
    case "svgText":
      return { x: 10, y: 20, text: "SVG Text", fill: "#111827" };
    case "svgGroup":
      return {};
    case "chartLine":
    case "chartBar":
    case "chartPie":
    case "chartDonut":
    case "chartArea":
    case "chartScatter":
      return {
        data: [{ x: 1, y: 10 }, { x: 2, y: 20 }],
        dataKey: "chartData",
      };
    case "codeEditor":
      return { language: "typescript", initialCode: "const x = 1;" };
    case "mathInline":
    case "mathBlock":
      return { formula: "\\sum_{i=1}^n i = \\frac{n(n+1)}{2}" };
    case "canvasScene":
      return { width: 300, height: 200, objects: [] };
    case "audioPlayer":
      return { src: "media/test.mp3", title: "Audio Track" };
    case "tabs":
      return {
        tabs: [
          { id: "tab-1", label: "Overview" },
          { id: "tab-2", label: "Details" },
        ],
      };
    case "dictationButton":
      return { label: "Dictate" };
    case "container":
    case "row":
    case "column":
    case "card":
    default:
      return {};
  }
}

describe("Component Gallery Exhaustive Coverage", () => {
  const allComponentTypes = ComponentTypeSchema.options;

  it("registers every schema component type and maps to a concrete implementation", () => {
    for (const type of allComponentTypes) {
      const Component = resolveComponent(type);
      expect(Component).toBeDefined();
      expect(Component).not.toBe(UnsupportedNode);
    }
  });

  it("renders every single component type in ComponentTypeSchema without throwing", () => {
    const components: ToolComponent[] = allComponentTypes.map((type, idx) => ({
      id: `gallery-${type}-${idx}`,
      type,
      props: samplePropsForType(type),
      children: type === "svgScene" ? [
        { id: `svg-child-${idx}`, type: "svgCircle", props: { cx: 20, cy: 20, r: 10 } }
      ] : type === "tabs" ? [
        { id: `tab-child-${idx}`, type: "text", props: { text: "Tab content" } }
      ] : undefined,
    }));

    const galleryTool: ToolDefinition = {
      id: "tool-component-gallery",
      name: "Component Gallery Test Fixture",
      description: "Comprehensive gallery exercising all ComponentTypeSchema nodes",
      layout: { type: "single-column" },
      components,
    };

    const { container } = render(
      <ToolRenderer
        tool={galleryTool}
        state={{
          tableData: [{ id: "1", name: "Alpha" }],
          chartData: [{ x: 1, y: 10 }],
        }}
        onStateChange={vi.fn()}
      />
    );

    // Verify all 64 component nodes rendered without error boundary crash
    expect(container.querySelector(".tr-error-boundary")).toBeNull();

    // Verify each node has its data-component-id
    for (const comp of components) {
      const el = container.querySelector(`[data-component-id="${comp.id}"]`);
      expect(el).not.toBeNull();
    }
  });
});
