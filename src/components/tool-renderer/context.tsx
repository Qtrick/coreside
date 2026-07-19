/* eslint-disable react-refresh/only-export-components -- provider + hook pair */
import {
  createContext,
  useContext,
  type ReactNode,
} from "react";
import type { ActionDefinition, ToolState } from "@/types/tool";

export type ToolRuntimeContextValue = {
  toolId: string;
  state: ToolState;
  runActions: (actions: ActionDefinition[], componentId?: string) => void;
  setValue: (key: string, value: unknown) => void;
  setValueOptimistic: (key: string, value: unknown) => void;
  getValue: <T = unknown>(key: string, fallback?: T) => T;
};

const ToolRuntimeContext = createContext<ToolRuntimeContextValue | null>(null);

export function ToolRuntimeProvider({
  value,
  children,
}: {
  value: ToolRuntimeContextValue;
  children: ReactNode;
}) {
  return (
    <ToolRuntimeContext.Provider value={value}>
      {children}
    </ToolRuntimeContext.Provider>
  );
}

export function useToolRuntime(): ToolRuntimeContextValue {
  const ctx = useContext(ToolRuntimeContext);
  if (!ctx) {
    throw new Error("useToolRuntime must be used within ToolRuntimeProvider");
  }
  return ctx;
}
