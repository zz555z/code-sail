import { createContext, useContext, type ReactNode } from "react";
import type { AppUpdateInfo, ToolStatus, ToolType } from "../lib/types";

export type AppServicesContextValue = {
  appVersion: string;
  appUpdate: AppUpdateInfo | null;
  checkingAppUpdate: boolean;
  openingAppUpdate: boolean;
  refreshAppUpdate: () => Promise<void>;
  openUpdatePage: () => Promise<void>;
  toolStatuses: ToolStatus[];
  toolStatusesLoading: boolean;
  openingTerminal: boolean;
  installingTool: string | null;
  refreshToolStatuses: () => Promise<void>;
  openInTerminal: (toolType: ToolType) => Promise<void>;
  openToolInstaller: (tool: ToolStatus) => Promise<void>;
};

const AppServicesContext = createContext<AppServicesContextValue | null>(null);

export function useAppServicesContext(): AppServicesContextValue {
  const ctx = useContext(AppServicesContext);
  if (!ctx) throw new Error("useAppServicesContext must be used within an AppServicesProvider");
  return ctx;
}

export function AppServicesProvider({
  value,
  children
}: {
  value: AppServicesContextValue;
  children: ReactNode;
}) {
  return <AppServicesContext.Provider value={value}>{children}</AppServicesContext.Provider>;
}
