import type { ToolDefinition, ToolSummary } from "@/types/tool";

/** Backend ToolRecord shape from Rust IPC. */
export type ToolRecord = {
  id: string;
  workspaceId?: string;
  name: string;
  description?: string;
  layout?: unknown;
  definition: ToolDefinition;
  currentVersion: number;
  createdAt?: string;
  updatedAt?: string;
};

export function isToolRecord(value: unknown): value is ToolRecord {
  return (
    typeof value === "object" &&
    value !== null &&
    "definition" in value &&
    "currentVersion" in value
  );
}

export function toToolDefinition(record: ToolRecord): ToolDefinition {
  return {
    ...record.definition,
    version: record.currentVersion,
  };
}

export function toToolSummary(record: ToolRecord): ToolSummary {
  return {
    id: record.id,
    name: record.name,
    description: record.description ?? record.definition.description ?? "",
    version: record.currentVersion,
    updatedAt: record.updatedAt ?? null,
    createdAt: record.createdAt ?? null,
  };
}
