import { invoke } from "@tauri-apps/api/core";
import type {
  AppState,
  CopyProviderResponse,
  DeleteHistoryResponse,
  FetchModelsResponse,
  HistoryConversation,
  HistoryProviderGroup,
  ProviderDraft,
  SaveProviderResponse,
  ToolStatus
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

export async function saveProvider(provider: ProviderDraft, updateConfig: boolean): Promise<SaveProviderResponse> {
  return await invokeCommand<SaveProviderResponse>("save_provider", {
    input: {
      ...provider,
      updateConfig
    }
  });
}

export async function copyProvider(providerId: string): Promise<CopyProviderResponse> {
  return await invokeCommand<CopyProviderResponse>("copy_provider", { providerId });
}

export async function deleteProvider(providerId: string): Promise<void> {
  await invokeCommand<void>("delete_provider", { providerId });
}

export async function fetchModels(provider: ProviderDraft): Promise<FetchModelsResponse> {
  return await invokeCommand<FetchModelsResponse>("fetch_models", {
    input: {
      originalId: provider.originalId,
      name: provider.name,
      baseUrl: provider.baseUrl,
      model: provider.model,
      token: provider.token || null
    }
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

export async function openToolInstall(command: string): Promise<void> {
  await invokeCommand<void>("open_tool_install", {
    input: { command }
  });
}

export async function listHistorySessions(): Promise<HistoryProviderGroup[]> {
  return await invokeCommand<HistoryProviderGroup[]>("list_history_sessions");
}

export async function readHistorySession(path: string): Promise<HistoryConversation> {
  return await invokeCommand<HistoryConversation>("read_history_session", {
    input: { path }
  });
}

export async function resumeHistorySession(sessionId: string): Promise<void> {
  await invokeCommand<void>("resume_history_session", {
    input: { sessionId }
  });
}

export async function deleteHistorySession(path: string): Promise<DeleteHistoryResponse> {
  return await invokeCommand<DeleteHistoryResponse>("delete_history_session", {
    input: { path }
  });
}

export async function deleteHistoryProvider(provider: string): Promise<DeleteHistoryResponse> {
  return await invokeCommand<DeleteHistoryResponse>("delete_history_provider", {
    input: { provider }
  });
}
