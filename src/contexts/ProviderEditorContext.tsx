import { createContext, useContext, type RefObject, type ReactNode } from "react";
import type { AppState, HealthCheckResponse, ProviderDraft, ProviderView } from "../lib/types";

type HealthStatus = HealthCheckResponse | "loading";

export type ProviderEditorContextValue = {
  state: AppState | null;
  selected: ProviderView | null;
  selectedId: string | null;
  draft: ProviderDraft;
  models: string[];
  modelValue: string;
  providerCount: number;
  activeProvider: ProviderView | null;
  editorOpen: boolean;
  busy: boolean;
  importingProviders: boolean;
  restarting: boolean;
  loadingModels: boolean;
  modelMenuOpen: boolean;
  tokenVisible: boolean;
  updateConfigFile: boolean;
  canSave: boolean;
  modelComboboxRef: RefObject<HTMLDivElement>;
  healthCheckResults: Record<string, HealthStatus>;
  healthCheckProvider: (provider: ProviderView) => Promise<void>;
  setUpdateConfigFile: (enabled: boolean) => void;
  setModelMenuOpen: (open: boolean | ((current: boolean) => boolean)) => void;
  setModelValue: (model: string) => void;
  toggleTokenVisible: () => void;
  refresh: (options?: { preferredId?: string | null }) => Promise<void>;
  restartCodex: () => Promise<void>;
  openCreateProvider: () => void;
  openEditProvider: (provider: ProviderView) => Promise<void>;
  copyProvider: (providerId: string) => Promise<void>;
  importFromCodexToClaude: () => Promise<void>;
  reorderProviders: (providerIds: string[]) => Promise<void>;
  setCurrentProvider: (provider: ProviderView) => Promise<void>;
  removeProvider: (providerId: string) => Promise<void>;
  closeEditor: () => void;
  updateDraft: (patch: Partial<ProviderDraft>) => void;
  selectModel: (model: string) => void;
  fetchProviderModels: () => Promise<void>;
  saveCurrentProvider: () => Promise<void>;
};

const ProviderEditorContext = createContext<ProviderEditorContextValue | null>(null);

export function useProviderEditorContext(): ProviderEditorContextValue {
  const ctx = useContext(ProviderEditorContext);
  if (!ctx) throw new Error("useProviderEditorContext must be used within a ProviderEditorProvider");
  return ctx;
}

export function ProviderEditorProvider({
  value,
  children
}: {
  value: ProviderEditorContextValue;
  children: ReactNode;
}) {
  return <ProviderEditorContext.Provider value={value}>{children}</ProviderEditorContext.Provider>;
}
