import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { ToolRenderer } from "@/components/tool-renderer/ToolRenderer";
import { FIXTURE_TOOLS } from "./fixture-tools";
import { verifySurfaceElement } from "@/lib/visual-verification";

describe("Visual Benchmark Suite", () => {
  const testWidths = [360, 480, 640, 800, 1024];

  for (const [name, tool] of Object.entries(FIXTURE_TOOLS)) {
    describe(`Fixture: ${name} (${tool.layout?.type ?? "stack"})`, () => {
      it("renders successfully without crashing and contains no fallback nodes", () => {
        const { container } = render(
          <div style={{ width: 800 }}>
            <ToolRenderer
              tool={tool}
              state={{}}
              onStateChange={() => {}}
            />
          </div>,
        );

        const surface = container.querySelector(".tr-layout-surface");
        expect(surface).not.toBeNull();

        const layoutType = tool.layout?.type ?? "stack";
        expect(surface?.classList.contains(`tr-layout-${layoutType}`)).toBe(true);

        const fallback = container.querySelector(".tr-fallback, .tr-unknown-component");
        expect(fallback).toBeNull();
      });

      it("assigns layout roles and spans to components", () => {
        const { container } = render(
          <div style={{ width: 800 }}>
            <ToolRenderer
              tool={tool}
              state={{}}
              onStateChange={() => {}}
            />
          </div>,
        );

        for (const comp of tool.components) {
          const el = container.querySelector(`[data-component-id="${comp.id}"]`);
          expect(el).not.toBeNull();
          const role = comp.layoutRole ?? comp.layout_role;
          if (role) {
            expect(el?.getAttribute("data-layout-role")).toBe(role);
          }
          const colSpan = comp.colSpan ?? comp.col_span;
          if (colSpan != null) {
            expect(el?.getAttribute("data-col-span")).toBe(String(colSpan));
          }
        }
      });

      for (const width of testWidths) {
        it(`passes visual verification at width ${width}px`, () => {
          const { container } = render(
            <div style={{ width: `${width}px` }}>
              <ToolRenderer
                tool={tool}
                state={{}}
                onStateChange={() => {}}
              />
            </div>,
          );

          const surface = container.querySelector(".tr-layout-surface") as HTMLElement;
          expect(surface).not.toBeNull();

          const result = verifySurfaceElement(surface, width);
          const overflowCheck = result.checks.find((c) => c.id === "no_horizontal_overflow");
          expect(overflowCheck?.status).toBe("pass");

          const labelCheck = result.checks.find((c) => c.id === "labels_present");
          expect(labelCheck?.status).toBe("pass");
        });
      }
    });
  }
});
