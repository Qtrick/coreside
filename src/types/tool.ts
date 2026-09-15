import { z } from "zod";

export const ComponentTypeSchema = z.enum([
  "container",
  "row",
  "column",
  "card",
  "tabs",
  "divider",
  "spacer",
  "heading",
  "text",
  "badge",
  "image",
  "emptyState",
  "textInput",
  "textArea",
  "numberInput",
  "select",
  "checkbox",
  "dateInput",
  "list",
  "checklist",
  "table",
  "counter",
  "progress",
  "stat",
  "button",
  "buttonGroup",
  "quiz",
  "clock",
  // Runtime V2 capability packs
  "form",
  "fieldGroup",
  "radioGroup",
  "slider",
  "switch",
  "colorInput",
  "timeInput",
  "dateTimeInput",
  "filePicker",
  "mediaPicker",
  "submitButton",
  "resetButton",
  "validationMessage",
  "svgScene",
  "svgRect",
  "svgCircle",
  "svgEllipse",
  "svgLine",
  "svgPath",
  "svgText",
  "svgGroup",
  "chartLine",
  "chartBar",
  "chartPie",
  "chartDonut",
  "chartArea",
  "chartScatter",
  "codeEditor",
  "mathInline",
  "mathBlock",
  "canvasScene",
  "audioPlayer",
  "dataTable",
  // Kept for parse compatibility with saved tools; not generatable (pack removed).
  "dictationButton",
]);

export type ComponentType = z.infer<typeof ComponentTypeSchema>;

export const ActionSchema = z.discriminatedUnion("type", [
  z.object({
    type: z.literal("setValue"),
    target: z.string().min(1),
    value: z.unknown(),
  }),
  z.object({
    type: z.literal("toggle"),
    target: z.string().min(1),
  }),
  z.object({
    type: z.literal("increment"),
    target: z.string().min(1),
    amount: z.number().optional(),
  }),
  z.object({
    type: z.literal("decrement"),
    target: z.string().min(1),
    amount: z.number().optional(),
  }),
  z.object({
    type: z.literal("reset"),
    target: z.string().min(1),
    value: z.unknown().optional(),
  }),
  z.object({
    type: z.literal("appendItem"),
    target: z.string().min(1),
    item: z.unknown(),
  }),
  z.object({
    type: z.literal("removeItem"),
    target: z.string().min(1),
    index: z.number().int().nonnegative().optional(),
    id: z.string().optional(),
  }),
  z.object({
    type: z.literal("updateItem"),
    target: z.string().min(1),
    index: z.number().int().nonnegative().optional(),
    id: z.string().optional(),
    patch: z.record(z.unknown()),
  }),
  z.object({
    type: z.literal("selectTab"),
    target: z.string().min(1),
    tabId: z.string().min(1),
  }),
  z.object({
    type: z.literal("submitToAgent"),
    eventName: z.string().min(1),
    includeFields: z.array(z.string()).optional(),
    componentId: z.string().optional(),
  }),
  z.object({
    type: z.literal("invokeRegisteredAction"),
    actionName: z.string().min(1),
    input: z.record(z.unknown()).optional(),
    inputFromState: z.record(z.string()).optional(),
    componentId: z.string().optional(),
  }),
]);

export type ActionDefinition = z.infer<typeof ActionSchema>;

export const LayoutRoleSchema = z.enum([
  "header",
  "stats",
  "main",
  "sidebar",
  "detail",
  "footer",
  "actions",
]);

export type LayoutRole = z.infer<typeof LayoutRoleSchema>;

export type ToolComponent = {
  id: string;
  type: ComponentType | string;
  props?: Record<string, unknown>;
  children?: ToolComponent[];
  actions?: ActionDefinition[];
  valueKey?: string;
  layoutRole?: LayoutRole | string;
  layout_role?: LayoutRole | string;
  colSpan?: number;
  col_span?: number;
  rowSpan?: number;
  row_span?: number;
};

export const ToolComponentSchema: z.ZodType<ToolComponent> = z.lazy(() =>
  z.object({
    id: z.string().min(1),
    type: z.string().min(1),
    props: z.record(z.unknown()).optional(),
    children: z.array(ToolComponentSchema).optional(),
    actions: z.array(ActionSchema).optional(),
    valueKey: z.string().optional(),
    layoutRole: z.string().optional(),
    layout_role: z.string().optional(),
    colSpan: z.number().int().min(1).max(12).optional(),
    col_span: z.number().int().min(1).max(12).optional(),
    rowSpan: z.number().int().min(1).max(6).optional(),
    row_span: z.number().int().min(1).max(6).optional(),
  }),
);

export const ToolLayoutTypeSchema = z.enum([
  "stack",
  "split",
  "grid",
  "dashboard",
  "form",
  "content",
  "full",
  "single-column", // legacy alias for stack
]);

export type ToolLayoutType = z.infer<typeof ToolLayoutTypeSchema>;

export const ToolLayoutSchema = z
  .object({
    type: ToolLayoutTypeSchema.default("stack"),
    columns: z.number().int().min(1).max(6).optional(),
    gap: z.enum(["none", "xs", "sm", "md", "lg", "xl"]).optional(),
    maxWidth: z.enum(["sm", "md", "lg", "xl", "full"]).optional(),
    density: z.enum(["compact", "normal", "comfortable"]).optional(),
    align: z.enum(["start", "center", "end", "stretch"]).optional(),
    splitRatio: z.enum(["1:1", "1:2", "1:3", "2:1", "3:1", "1:4", "4:1"]).optional(),
    collapseAt: z.enum(["mobile", "tablet", "never"]).optional(),
  })
  .passthrough()
  .optional();

export type ToolLayout = z.infer<typeof ToolLayoutSchema>;

export const ToolDefinitionSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  description: z.string().default(""),
  layout: ToolLayoutSchema,
  components: z.array(ToolComponentSchema).default([]),
  version: z.number().int().positive().optional(),
});

export type ToolDefinition = z.infer<typeof ToolDefinitionSchema>;

export const ToolSummarySchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string().optional().nullable(),
  version: z.number().int().optional().nullable(),
  updatedAt: z.string().optional().nullable(),
  createdAt: z.string().optional().nullable(),
});

export type ToolSummary = z.infer<typeof ToolSummarySchema>;

export const ToolVersionSchema = z.object({
  id: z.string(),
  toolId: z.string(),
  version: z.number().int(),
  changeSummary: z.string().optional().nullable(),
  createdAt: z.string(),
  definition: ToolDefinitionSchema.optional(),
});

export type ToolVersion = z.infer<typeof ToolVersionSchema>;

export type ToolState = Record<string, unknown>;

export const ToolInteractionEventSchema = z.object({
  eventType: z.literal("tool_interaction"),
  toolId: z.string(),
  componentId: z.string().optional(),
  eventName: z.string(),
  values: z.record(z.unknown()),
});

export type ToolInteractionEvent = z.infer<typeof ToolInteractionEventSchema>;
