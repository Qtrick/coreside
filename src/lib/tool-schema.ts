import {
  ActionSchema,
  ComponentTypeSchema,
  ToolComponentSchema,
  ToolDefinitionSchema,
  type ActionDefinition,
  type ToolComponent,
  type ToolDefinition,
} from "@/types/tool";

const SAFE_ID = /^[a-zA-Z][a-zA-Z0-9_-]{0,63}$/;

export function isSupportedComponentType(type: string): boolean {
  return ComponentTypeSchema.safeParse(type).success;
}

export function validateToolDefinition(input: unknown): {
  success: true;
  data: ToolDefinition;
  warnings: string[];
} | {
  success: false;
  error: string;
  issues: string[];
} {
  const parsed = ToolDefinitionSchema.safeParse(input);
  if (!parsed.success) {
    return {
      success: false,
      error: "Invalid tool definition",
      issues: parsed.error.issues.map(
        (i) => `${i.path.join(".") || "root"}: ${i.message}`,
      ),
    };
  }

  const warnings: string[] = [];
  const seen = new Set<string>();

  const walk = (components: ToolComponent[], path: string) => {
    for (const [index, component] of components.entries()) {
      const here = `${path}[${index}]`;
      if (!SAFE_ID.test(component.id)) {
        warnings.push(`${here}: id "${component.id}" should be a stable safe identifier`);
      }
      if (seen.has(component.id)) {
        warnings.push(`${here}: duplicate component id "${component.id}"`);
      } else {
        seen.add(component.id);
      }
      if (!isSupportedComponentType(component.type)) {
        warnings.push(
          `${here}: unsupported component type "${component.type}"`,
        );
      }
      if (component.actions) {
        for (const [aIndex, action] of component.actions.entries()) {
          const actionResult = ActionSchema.safeParse(action);
          if (!actionResult.success) {
            warnings.push(
              `${here}.actions[${aIndex}]: ${actionResult.error.issues[0]?.message ?? "invalid action"}`,
            );
          }
        }
      }
      if (component.children?.length) {
        walk(component.children, `${here}.children`);
      }
    }
  };

  walk(parsed.data.components, "components");

  return { success: true, data: parsed.data, warnings };
}

export function validateAction(input: unknown): ActionDefinition | null {
  const result = ActionSchema.safeParse(input);
  return result.success ? result.data : null;
}

export function parseToolComponent(input: unknown): ToolComponent | null {
  const result = ToolComponentSchema.safeParse(input);
  return result.success ? result.data : null;
}

export { ToolDefinitionSchema, ToolComponentSchema, ActionSchema, ComponentTypeSchema };
