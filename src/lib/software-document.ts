/**
 * Software Document (UI Intermediate Representation)
 *
 * A declarative, structured representation of personal software in Coreside.
 * Provides stable, addressable section and component targets for incremental
 * model mutations without permitting arbitrary HTML or JavaScript execution.
 *
 * Rust authority: src-tauri/src/runtime_v2/software_document.rs
 */

import { z } from "zod";
import type { ActionDefinition, ToolComponent, ToolDefinition } from "@/types/tool";

export const StateScopeSchema = z.enum([
  "persistent",
  "session",
  "ephemeral",
  "in_flight",
  "error",
]);

export type StateScope = z.infer<typeof StateScopeSchema>;

export const DocumentSectionSchema = z.object({
  id: z.string().min(1),
  title: z.string().optional(),
  role: z.string().optional(),
  layout: z.string().optional(),
  parentRegionId: z.string().optional(),
  slot: z.string().optional(),
  responsive: z.record(z.unknown()).optional(),
  instanceId: z.string().optional(),
  components: z.array(z.any()).default([]),
  metadata: z.record(z.unknown()).optional(),
});

export type DocumentSection = {
  id: string;
  title?: string;
  role?: string;
  layout?: string;
  parentRegionId?: string;
  slot?: string;
  responsive?: Record<string, unknown>;
  instanceId?: string;
  components: ToolComponent[];
  metadata?: Record<string, unknown>;
};

export const StateContractSchema = z.object({
  key: z.string().min(1),
  type: z.string().default("string"),
  initialValue: z.unknown().default(null),
  description: z.string().optional(),
  scope: StateScopeSchema.default("persistent"),
  preservationPolicy: z.string().optional(),
});

export type StateContract = z.infer<typeof StateContractSchema>;

export const ActionContractSchema = z.object({
  actionId: z.string().min(1),
  actionName: z.string().min(1),
  description: z.string().optional(),
  resultKey: z.string().optional(),
  inputFromState: z.record(z.string()).optional(),
});

export type ActionContract = z.infer<typeof ActionContractSchema>;

export const RepairNoteSchema = z.object({
  kind: z.string(),
  targetId: z.string(),
  detail: z.string(),
});

export type RepairNote = z.infer<typeof RepairNoteSchema>;

export const SoftwareDocumentSchema = z.object({
  id: z.string().min(1),
  title: z.string().min(1),
  description: z.string().optional(),
  version: z.number().int().positive().default(1),
  sections: z.array(DocumentSectionSchema).default([]),
  stateContracts: z.array(StateContractSchema).default([]),
  actionContracts: z.array(ActionContractSchema).default([]),
  designTokens: z.record(z.unknown()).optional(),
  capabilityPacks: z.array(z.string()).default(["core", "base-layout"]),
});

export type SoftwareDocument = {
  id: string;
  title: string;
  description?: string;
  version: number;
  sections: DocumentSection[];
  stateContracts: StateContract[];
  actionContracts: ActionContract[];
  designTokens?: Record<string, unknown>;
  capabilityPacks: string[];
};

/**
 * Convert a flat ToolDefinition into a structured SoftwareDocument.
 */
export function fromToolDefinition(tool: ToolDefinition): SoftwareDocument {
  const doc: SoftwareDocument = {
    id: tool.id,
    title: tool.name,
    description: tool.description ? tool.description : undefined,
    version: tool.version ?? 1,
    sections: [],
    stateContracts: [],
    actionContracts: [],
    designTokens: undefined,
    capabilityPacks: ["core", "base-layout"],
  };

  const sectionMap = new Map<string, ToolComponent[]>();
  const sectionOrder: string[] = [];

  for (const comp of tool.components || []) {
    const secId =
      comp.props && typeof comp.props.section === "string" && comp.props.section.trim() !== ""
        ? comp.props.section.trim()
        : "main";

    if (!sectionMap.has(secId)) {
      sectionOrder.push(secId);
      sectionMap.set(secId, []);
    }
    sectionMap.get(secId)!.push(comp);
  }

  for (const secId of sectionOrder) {
    const comps = sectionMap.get(secId) || [];
    doc.sections.push({
      id: secId,
      title: secId === "main" ? undefined : secId,
      role: "content",
      layout: "stack",
      components: comps,
    });
  }

  return doc;
}

/**
 * Render/flatten SoftwareDocument into a ToolDefinition for runtime presentation.
 */
export function toToolDefinition(doc: SoftwareDocument): ToolDefinition {
  const allComponents: ToolComponent[] = [];

  for (const section of doc.sections) {
    for (const comp of section.components) {
      const c = { ...comp };
      const props = { ...(c.props || {}) };
      if (!props.section) {
        props.section = section.id;
      }
      c.props = props;
      allComponents.push(c);
    }
  }

  return {
    id: doc.id,
    name: doc.title,
    description: doc.description || "",
    layout: { type: "single-column" },
    components: allComponents,
    version: doc.version,
  };
}

/**
 * Self-repair helper for sanitizing component trees, stripping unsafe handlers,
 * deduplicating IDs, and populating state contracts.
 */
export function validateAndRepair(doc: SoftwareDocument): {
  doc: SoftwareDocument;
  notes: RepairNote[];
} {
  const cloned: SoftwareDocument = JSON.parse(JSON.stringify(doc));
  const notes: RepairNote[] = [];
  const seenComponentIds = new Set<string>();
  const declaredStateKeys = new Set(cloned.stateContracts.map((s) => s.key));

  if (cloned.sections.length === 0) {
    cloned.sections.push({
      id: "main",
      role: "content",
      layout: "stack",
      components: [],
    });
    notes.push({
      kind: "added_fallback_section",
      targetId: "main",
      detail: "Created fallback 'main' section for empty document",
    });
  }

  function repairComponent(comp: ToolComponent) {
    // 1. Deduplicate ID
    if (!comp.id || comp.id.trim() === "") {
      comp.id = `comp-${Math.random().toString(36).slice(2, 8)}`;
      notes.push({
        kind: "generated_component_id",
        targetId: comp.id,
        detail: "Generated missing component ID",
      });
    } else if (seenComponentIds.has(comp.id)) {
      const oldId = comp.id;
      comp.id = `${oldId}-${Math.random().toString(36).slice(2, 6)}`;
      notes.push({
        kind: "deduplicated_component_id",
        targetId: comp.id,
        detail: `Renamed duplicate component ID '${oldId}' to '${comp.id}'`,
      });
    }
    seenComponentIds.add(comp.id);

    // 2. Sanitize props (strip dangerous on* handlers and javascript: strings)
    if (comp.props && typeof comp.props === "object") {
      for (const key of Object.keys(comp.props)) {
        if (key.toLowerCase().startsWith("on") && typeof comp.props[key] === "string") {
          delete comp.props[key];
          notes.push({
            kind: "removed_event_handler",
            targetId: comp.id,
            detail: `Removed unsafe inline event handler '${key}'`,
          });
        } else if (typeof comp.props[key] === "string") {
          const val = comp.props[key] as string;
          if (
            val.toLowerCase().includes("javascript:") ||
            val.toLowerCase().includes("<script")
          ) {
            delete comp.props[key];
            notes.push({
              kind: "removed_script_injection",
              targetId: comp.id,
              detail: `Removed potential script payload in prop '${key}'`,
            });
          }
        }
      }
    }

    // 3. Auto-synthesize missing state contract
    if (comp.valueKey && comp.valueKey.trim() !== "") {
      const vk = comp.valueKey.trim();
      if (!declaredStateKeys.has(vk)) {
        declaredStateKeys.add(vk);
        cloned.stateContracts.push({
          key: vk,
          type: "string",
          initialValue: null,
          scope: "persistent",
          description: `Auto-declared from component '${comp.id}'`,
        });
        notes.push({
          kind: "auto_declared_state_contract",
          targetId: vk,
          detail: `Synthesized missing StateContract for '${vk}'`,
        });
      }
    }

    // 4. Recurse children
    if (Array.isArray(comp.children)) {
      for (const child of comp.children) {
        repairComponent(child);
      }
    }
  }

  for (const section of cloned.sections) {
    for (const comp of section.components) {
      repairComponent(comp);
    }
  }

  return { doc: cloned, notes };
}

/**
 * Add a section to the software document.
 */
export function addSection(
  doc: SoftwareDocument,
  section: DocumentSection,
  index?: number
): SoftwareDocument {
  if (!section.id || section.id.trim() === "") {
    throw new Error("section id cannot be empty");
  }
  if (doc.sections.some((s) => s.id === section.id)) {
    throw new Error(`section with id '${section.id}' already exists`);
  }

  const cloned: SoftwareDocument = JSON.parse(JSON.stringify(doc));
  const newSec: DocumentSection = {
    role: "content",
    layout: "stack",
    ...section,
  };

  if (typeof index === "number" && index >= 0 && index <= cloned.sections.length) {
    cloned.sections.splice(index, 0, newSec);
  } else {
    cloned.sections.push(newSec);
  }

  return cloned;
}

/**
 * Remove a section by its ID.
 */
export function removeSection(doc: SoftwareDocument, sectionId: string): SoftwareDocument {
  const cloned: SoftwareDocument = JSON.parse(JSON.stringify(doc));
  const idx = cloned.sections.findIndex((s) => s.id === sectionId);
  if (idx === -1) {
    throw new Error(`section '${sectionId}' not found`);
  }
  cloned.sections.splice(idx, 1);
  return cloned;
}

/**
 * Update a section by ID.
 */
export function updateSection(
  doc: SoftwareDocument,
  sectionId: string,
  updates: Partial<Omit<DocumentSection, "id">>
): SoftwareDocument {
  const cloned: SoftwareDocument = JSON.parse(JSON.stringify(doc));
  const sec = cloned.sections.find((s) => s.id === sectionId);
  if (!sec) {
    throw new Error(`section '${sectionId}' not found`);
  }
  if (updates.title !== undefined) sec.title = updates.title;
  if (updates.role !== undefined) sec.role = updates.role;
  if (updates.layout !== undefined) sec.layout = updates.layout;
  if (updates.components !== undefined) sec.components = updates.components;
  if (updates.metadata !== undefined) sec.metadata = updates.metadata;

  return cloned;
}

/**
 * Bind a component to a state contract.
 */
export function bindState(
  doc: SoftwareDocument,
  componentId: string,
  key: string,
  initialValue: unknown = null
): SoftwareDocument {
  const cloned: SoftwareDocument = JSON.parse(JSON.stringify(doc));
  let found = false;

  function visit(comp: ToolComponent) {
    if (comp.id === componentId) {
      comp.valueKey = key;
      found = true;
      return;
    }
    if (comp.children) {
      for (const child of comp.children) {
        visit(child);
        if (found) return;
      }
    }
  }

  for (const sec of cloned.sections) {
    for (const comp of sec.components) {
      visit(comp);
      if (found) break;
    }
    if (found) break;
  }

  if (!found) {
    throw new Error(`component '${componentId}' not found`);
  }

  if (!cloned.stateContracts.some((sc) => sc.key === key)) {
    cloned.stateContracts.push({
      key,
      type: "string",
      initialValue,
      scope: "persistent",
      description: `Bound to component '${componentId}'`,
    });
  }

  return cloned;
}

/**
 * Bind an action to a component.
 */
export function bindAction(
  doc: SoftwareDocument,
  componentId: string,
  action: ActionDefinition
): SoftwareDocument {
  const cloned: SoftwareDocument = JSON.parse(JSON.stringify(doc));
  let found = false;

  function visit(comp: ToolComponent) {
    if (comp.id === componentId) {
      if (!comp.actions) comp.actions = [];
      comp.actions.push(action);
      found = true;
      return;
    }
    if (comp.children) {
      for (const child of comp.children) {
        visit(child);
        if (found) return;
      }
    }
  }

  for (const sec of cloned.sections) {
    for (const comp of sec.components) {
      visit(comp);
      if (found) break;
    }
    if (found) break;
  }

  if (!found) {
    throw new Error(`component '${componentId}' not found`);
  }

  if (action.type === "invokeRegisteredAction") {
    const actId = `${componentId}-${action.actionName.replace(/\./g, "-")}`;
    if (!cloned.actionContracts.some((ac) => ac.actionId === actId)) {
      cloned.actionContracts.push({
        actionId: actId,
        actionName: action.actionName,
        description: `Triggered from component '${componentId}'`,
        resultKey: action.resultKey,
        inputFromState: action.inputFromState,
      });
    }
  }

  return cloned;
}

/**
 * Set a safe design token or style property.
 */
export function setStyleToken(
  doc: SoftwareDocument,
  targetId: string,
  token: string,
  value: unknown
): SoftwareDocument {
  const cloned: SoftwareDocument = JSON.parse(JSON.stringify(doc));

  // 1. Check if section
  const sec = cloned.sections.find((s) => s.id === targetId);
  if (sec) {
    sec.metadata = { ...(sec.metadata || {}), [token]: value };
    return cloned;
  }

  // 2. Check if component
  let found = false;
  function visit(comp: ToolComponent) {
    if (comp.id === targetId) {
      comp.props = { ...(comp.props || {}), [token]: value };
      found = true;
      return;
    }
    if (comp.children) {
      for (const child of comp.children) {
        visit(child);
        if (found) return;
      }
    }
  }

  for (const s of cloned.sections) {
    for (const comp of s.components) {
      visit(comp);
      if (found) break;
    }
    if (found) break;
  }

  if (!found) {
    throw new Error(`target '${targetId}' not found`);
  }

  return cloned;
}

/**
 * Find a section/region by ID.
 */
export function findRegion(
  doc: SoftwareDocument,
  regionId: string
): DocumentSection | undefined {
  return doc.sections.find((s) => s.id === regionId);
}

/**
 * Get all child regions of a parent region.
 */
export function getChildRegions(
  doc: SoftwareDocument,
  parentRegionId: string
): DocumentSection[] {
  return doc.sections.filter((s) => s.parentRegionId === parentRegionId);
}

/**
 * Get the hierarchical address path for a region (e.g. "doc-1/main/sidebar").
 */
export function getRegionPath(
  doc: SoftwareDocument,
  regionId: string
): string | undefined {
  let curr = findRegion(doc, regionId);
  if (!curr) return undefined;

  const segments: string[] = [curr.id];
  while (curr.parentRegionId) {
    const parent = findRegion(doc, curr.parentRegionId);
    if (parent) {
      segments.push(parent.id);
      curr = parent;
    } else {
      break;
    }
  }
  segments.push(doc.id);
  segments.reverse();
  return segments.join("/");
}

/**
 * Find a component along with its enclosing section/region.
 */
export function findComponentWithRegion(
  doc: SoftwareDocument,
  componentId: string
): { section: DocumentSection; component: ToolComponent } | undefined {
  for (const section of doc.sections) {
    for (const comp of section.components) {
      if (comp.id === componentId) {
        return { section, component: comp };
      }
    }
  }
  return undefined;
}

/**
 * Move a component from one region to another with optional target index.
 */
export function moveComponent(
  doc: SoftwareDocument,
  componentId: string,
  targetRegionId: string,
  targetIndex?: number
): SoftwareDocument {
  const cloned: SoftwareDocument = JSON.parse(JSON.stringify(doc));

  let extracted: ToolComponent | undefined;
  for (const sec of cloned.sections) {
    const pos = sec.components.findIndex((c) => c.id === componentId);
    if (pos !== -1) {
      extracted = sec.components.splice(pos, 1)[0];
      break;
    }
  }

  if (!extracted) {
    throw new Error(`component '${componentId}' not found`);
  }

  if (extracted.props && typeof extracted.props === "object") {
    extracted.props.section = targetRegionId;
  }

  const targetSec = cloned.sections.find((s) => s.id === targetRegionId);
  if (!targetSec) {
    throw new Error(`target region '${targetRegionId}' not found`);
  }

  if (
    typeof targetIndex === "number" &&
    targetIndex >= 0 &&
    targetIndex <= targetSec.components.length
  ) {
    targetSec.components.splice(targetIndex, 0, extracted);
  } else {
    targetSec.components.push(extracted);
  }

  return cloned;
}

/**
 * Update layout and responsive configurations for a region.
 */
export function updateRegionLayout(
  doc: SoftwareDocument,
  regionId: string,
  layout?: string,
  responsive?: Record<string, unknown>
): SoftwareDocument {
  const cloned: SoftwareDocument = JSON.parse(JSON.stringify(doc));
  const sec = cloned.sections.find((s) => s.id === regionId);
  if (!sec) {
    throw new Error(`region '${regionId}' not found`);
  }
  if (layout !== undefined) sec.layout = layout;
  if (responsive !== undefined) sec.responsive = responsive;
  return cloned;
}

/**
 * Filter state by explicit scope.
 */
export function filterStateByScope(
  doc: SoftwareDocument,
  activeState: Record<string, unknown>,
  scope: StateScope
): Record<string, unknown> {
  const filtered: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(activeState)) {
    const contract = doc.stateContracts.find((sc) => sc.key === k);
    if (contract && contract.scope === scope) {
      filtered[k] = v;
    }
  }
  return filtered;
}

/**
 * Filter and preserve state values according to state contracts.
 * Ephemeral and InFlight states are discarded, while Persistent and Session states are kept.
 */
export function preserveStateValues(
  doc: SoftwareDocument,
  previousState: Record<string, unknown>
): Record<string, unknown> {
  const preserved: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(previousState)) {
    const contract = doc.stateContracts.find((sc) => sc.key === k);
    if (contract) {
      if (contract.scope !== "ephemeral" && contract.scope !== "in_flight") {
        preserved[k] = v;
      }
    } else {
      // Unspecified keys are preserved by default for safety
      preserved[k] = v;
    }
  }
  return preserved;
}

