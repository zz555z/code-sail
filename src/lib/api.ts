import { invoke } from "@tauri-apps/api/core";
import type {
  AppState,
  AppUpdateInfo,
  CopyProviderResponse,
  DeleteHistoryResponse,
  FetchModelsResponse,
  HealthCheckResponse,
  HistoryConversation,
  HistoryProviderGroup,
  ImportProvidersResponse,
  ProviderDetail,
  ProviderDraft,
  SaveProviderResponse,
  ToolStatus,
  ToolType
} from "./types";

async function invokeCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(error instanceof Error ? error.message : String(error));
  }
}

export async function getAppState(): Promise<AppState> {
  return await invokeCommand<AppState>("get_app_state");
}

export async function getToolStatuses(): Promise<ToolStatus[]> {
  return await invokeCommand<ToolStatus[]>("get_tool_statuses");
}

export async function checkAppUpdate(currentVersion: string): Promise<AppUpdateInfo> {
  return await invokeCommand<AppUpdateInfo>("check_app_update", { currentVersion });
}

export async function openAppUpdate(): Promise<void> {
  await invokeCommand<void>("open_app_update");
}

export async function saveProvider(provider: ProviderDraft, updateConfig: boolean): Promise<SaveProviderResponse> {
  return await invokeCommand<SaveProviderResponse>("save_provider", {
    input: {
      ...provider,
      updateConfig,
      toolType: provider.toolType
    }
  });
}

export async function getProviderDetail(providerId: string): Promise<ProviderDetail> {
  return await invokeCommand<ProviderDetail>("get_provider_detail", { providerId });
}

export async function copyProvider(providerId: string): Promise<CopyProviderResponse> {
  return await invokeCommand<CopyProviderResponse>("copy_provider", { providerId });
}

export async function importCodexProvidersToClaude(): Promise<ImportProvidersResponse> {
  return await invokeCommand<ImportProvidersResponse>("import_codex_providers_to_claude");
}

export async function deleteProvider(providerId: string): Promise<void> {
  await invokeCommand<void>("delete_provider", { providerId });
}

export async function reorderProviders(providerIds: string[]): Promise<void> {
  await invokeCommand<void>("reorder_providers", { providerIds });
}

export async function fetchModels(provider: ProviderDraft): Promise<FetchModelsResponse> {
  return await invokeCommand<FetchModelsResponse>("fetch_models", {
    input: {
      originalId: provider.originalId,
      baseUrl: provider.baseUrl,
      token: provider.token || null,
      toolType: provider.toolType
    }
  });
}

export async function checkProviderHealth(
  baseUrl: string,
  providerId: string
): Promise<HealthCheckResponse> {
  return await invokeCommand<HealthCheckResponse>("check_provider_health", {
    input: { baseUrl, providerId }
  });
}

export async function setCurrentModel(
  providerId: string,
  model: string,
  token: string,
  updateConfig: boolean
): Promise<void> {
  await invokeCommand<void>("set_current_model", {
    input: {
      providerId,
      model,
      token: token || null,
      updateConfig
    }
  });
}

export async function restartCodexApp(): Promise<void> {
  await invokeCommand<void>("restart_codex_app");
}

export async function openCodexTerminal(): Promise<void> {
  await invokeCommand<void>("open_codex_terminal");
}

export async function openClaudeTerminal(): Promise<void> {
  await invokeCommand<void>("open_claude_terminal");
}

export async function openToolInstall(command: string): Promise<void> {
  await invokeCommand<void>("open_tool_install", {
    input: { command }
  });
}

export async function listHistorySessions(toolType: ToolType): Promise<HistoryProviderGroup[]> {
  return await invokeCommand<HistoryProviderGroup[]>("list_tool_history_sessions", { toolType });
}

export async function readHistorySession(path: string, toolType: ToolType): Promise<HistoryConversation> {
  return await invokeCommand<HistoryConversation>("read_history_session", {
    input: { path, toolType }
  });
}

export async function resumeHistorySession(sessionId: string, toolType: ToolType): Promise<void> {
  await invokeCommand<void>("resume_history_session", {
    input: { sessionId, toolType }
  });
}

export async function deleteHistorySession(path: string, toolType: ToolType): Promise<DeleteHistoryResponse> {
  return await invokeCommand<DeleteHistoryResponse>("delete_history_session", {
    input: { path, toolType }
  });
}

export async function deleteHistoryProvider(provider: string, toolType: ToolType): Promise<DeleteHistoryResponse> {
  return await invokeCommand<DeleteHistoryResponse>("delete_history_provider", {
    input: { provider, toolType }
  });
}

export async function getActiveTool(): Promise<ToolType> {
  return await invokeCommand<ToolType>("get_active_tool_command");
}

export async function setActiveTool(toolType: ToolType): Promise<void> {
  await invokeCommand<void>("set_active_tool_command", { toolType });
}
