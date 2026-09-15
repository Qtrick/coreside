import React, { useMemo } from "react";
import type { ToolComponent, ToolLayout, ToolLayoutType } from "@/types/tool";

export interface ToolLayoutContainerProps {
  layout?: ToolLayout;
  components: ToolComponent[];
  renderComponent: (component: ToolComponent) => React.ReactNode;
  toolId: string;
  isCustomizing?: boolean;
  selectedComponentId?: string | null;
  onSelectComponent?: (component: ToolComponent) => void;
}

export function ToolLayoutContainer({
  layout,
  components,
  renderComponent,
  toolId,
  isCustomizing,
  selectedComponentId,
  onSelectComponent,
}: ToolLayoutContainerProps) {
  const rawType = layout?.type ?? "stack";
  const layoutType: ToolLayoutType = rawType === "single-column" ? "stack" : rawType;
  const gap = layout?.gap ?? "md";
  const maxWidth = layout?.maxWidth ?? (layoutType === "content" ? "md" : "full");
  const density = layout?.density ?? "normal";
  const align = layout?.align ?? "stretch";
  const columns = layout?.columns ?? 3;
  const splitRatio = layout?.splitRatio ?? "1:2";

  // Categorize components for archetype layouts if layoutRoles are used
  const { headerNodes, statsNodes, mainNodes, sidebarNodes, footerNodes, defaultNodes } =
    useMemo(() => {
      const header: ToolComponent[] = [];
      const stats: ToolComponent[] = [];
      const main: ToolComponent[] = [];
      const sidebar: ToolComponent[] = [];
      const footer: ToolComponent[] = [];
      const def: ToolComponent[] = [];

      for (const comp of components) {
        const role = comp.layoutRole ?? comp.layout_role;
        switch (role) {
          case "header":
            header.push(comp);
            break;
          case "stat":
          case "stats":
            stats.push(comp);
            break;
          case "main":
            main.push(comp);
            break;
          case "sidebar":
          case "detail":
            sidebar.push(comp);
            break;
          case "footer":
          case "actions":
            footer.push(comp);
            break;
          default:
            def.push(comp);
            break;
        }
      }
      return {
        headerNodes: header,
        statsNodes: stats,
        mainNodes: main,
        sidebarNodes: sidebar,
        footerNodes: footer,
        defaultNodes: def,
      };
    }, [components]);

  const renderItem = (component: ToolComponent, extraClass = "") => {
    const isSelected = selectedComponentId === component.id;
    const role = component.layoutRole ?? component.layout_role;
    const colSpan = component.colSpan ?? component.col_span;
    const rowSpan = component.rowSpan ?? component.row_span;

    return (
      <div
        key={component.id}
        className={`tr-layout-item ${extraClass}${isSelected ? " tr-item-selected" : ""}${
          isCustomizing ? " tr-customizing-interactive" : ""
        }`}
        data-component-id={component.id}
        data-component-type={component.type}
        data-layout-role={role}
        data-col-span={colSpan}
        data-row-span={rowSpan}
        data-item-col-span={colSpan}
        data-item-row-span={rowSpan}
        style={
          {
            "--item-col-span": colSpan ? String(colSpan) : undefined,
            "--item-row-span": rowSpan ? String(rowSpan) : undefined,
          } as React.CSSProperties
        }
        onClick={
          isCustomizing
            ? (e) => {
                e.stopPropagation();
                onSelectComponent?.(component);
              }
            : undefined
        }
      >
        {isCustomizing && (
          <div className="tr-item-badge" aria-hidden>
            <span className="tr-item-type">{component.type}</span>
            {role && (
              <span className="tr-item-role">{role}</span>
            )}
          </div>
        )}
        {renderComponent(component)}
      </div>
    );
  };

  const containerStyle = {
    "--layout-cols": String(columns),
    "--split-ratio": splitRatio.replace(":", "fr ") + "fr",
  } as React.CSSProperties;

  return (
    <div
      className={`tr-layout-surface tr-layout-${layoutType} tr-gap-${gap} tr-max-w-${maxWidth} tr-density-${density} tr-align-${align}`}
      data-tool-id={toolId}
      data-layout-type={layoutType}
      data-customizing={isCustomizing ? "true" : undefined}
      style={containerStyle}
    >
      {layoutType === "dashboard" && (headerNodes.length > 0 || statsNodes.length > 0) ? (
        <>
          {headerNodes.length > 0 && (
            <div className="tr-dashboard-header">
              {headerNodes.map((c) => renderItem(c, "tr-role-header"))}
            </div>
          )}
          {statsNodes.length > 0 && (
            <div className="tr-dashboard-stats">
              {statsNodes.map((c) => renderItem(c, "tr-role-stats"))}
            </div>
          )}
          <div className="tr-dashboard-body">
            <div className="tr-dashboard-main">
              {mainNodes.map((c) => renderItem(c, "tr-role-main"))}
              {defaultNodes.map((c) => renderItem(c))}
            </div>
            {sidebarNodes.length > 0 && (
              <div className="tr-dashboard-sidebar">
                {sidebarNodes.map((c) => renderItem(c, "tr-role-sidebar"))}
              </div>
            )}
          </div>
          {footerNodes.length > 0 && (
            <div className="tr-dashboard-footer">
              {footerNodes.map((c) => renderItem(c, "tr-role-footer"))}
            </div>
          )}
        </>
      ) : layoutType === "split" && (sidebarNodes.length > 0 || mainNodes.length > 0 || headerNodes.length > 0 || footerNodes.length > 0) ? (
        <>
          {headerNodes.length > 0 && (
            <div className="tr-split-header">
              {headerNodes.map((c) => renderItem(c, "tr-role-header"))}
            </div>
          )}
          {splitRatio.startsWith("1:") && splitRatio !== "1:1" ? (
            <>
              <div className="tr-split-sidebar">
                {sidebarNodes.map((c) => renderItem(c, "tr-role-sidebar"))}
              </div>
              <div className="tr-split-main">
                {mainNodes.map((c) => renderItem(c, "tr-role-main"))}
                {defaultNodes.map((c) => renderItem(c))}
              </div>
            </>
          ) : (
            <>
              <div className="tr-split-main">
                {mainNodes.map((c) => renderItem(c, "tr-role-main"))}
                {defaultNodes.map((c) => renderItem(c))}
              </div>
              <div className="tr-split-sidebar">
                {sidebarNodes.map((c) => renderItem(c, "tr-role-sidebar"))}
              </div>
            </>
          )}
          {footerNodes.length > 0 && (
            <div className="tr-split-footer">
              {footerNodes.map((c) => renderItem(c, "tr-role-footer"))}
            </div>
          )}
        </>
      ) : (
        components.map((c) => renderItem(c))
      )}
    </div>
  );
}
