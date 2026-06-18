import { createContext, useContext, type ReactNode } from "react";
import type { ToolType } from "../lib/types";

export type ActiveToolContextValue = {
  activeTool: ToolType;
  switching: boolean;
  switchTool: (tool: ToolType) => Promise<void>;
};

const ActiveToolContext = createContext<ActiveToolContextValue | null>(null);

export function useActiveToolContext(): ActiveToolContextValue {
  const ctx = useContext(ActiveToolContext);
  if (!ctx) throw new Error("useActiveToolContext must be used within an ActiveToolProvider");
  return ctx;
}

export function ActiveToolProvider({
  value,
  children
}: {
  value: ActiveToolContextValue;
  children: ReactNode;
}) {
  return <ActiveToolContext.Provider value={value}>{children}</ActiveToolContext.Provider>;
}
