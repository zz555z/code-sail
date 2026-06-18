// Type definitions matching Rust structs in src-tauri/src/
// Manually maintained — update when Rust structs change

export type ToolType = "codex" | "claude";

export type ProviderView = {
  id: string;
  name: string | null;
  baseUrl: string | null;
  model: string | null;
  models: string[];
  tokenPresent: boolean;
  toolType: ToolType;
};

export type ProviderDetail = ProviderView & {
  token: string | null;
};

export type AppState = {
  configPath: string;
  configExists: boolean;
  activeProvider: string | null;
  activeModel: string | null;
  providers: ProviderView[];
  activeTool: ToolType;
};

export type ToolStatus = {
  name: string;
  command: string;
  available: boolean;
  version: string | null;
  detail: string | null;
  installLabel: string;
  installHint: string;
  installUrl: string;
};

export type AppUpdateInfo = {
  currentVersion: string;
  latestVersion: string | null;
  updateAvailable: boolean;
  releaseUrl: string;
  releaseName: string | null;
  publishedAt: string | null;
  detail: string | null;
};

export type ProviderDraft = {
  originalId: string | null;
  name: string;
  baseUrl: string;
  model: string;
  token: string;
  toolType: ToolType;
};

export type FetchModelsResponse = {
  providerId: string;
  models: string[];
};

export type SaveProviderResponse = {
  providerId: string;
};

export type CopyProviderResponse = {
  providerId: string;
};

export type ImportProvidersResponse = {
  importedCount: number;
};

export type HealthCheckResponse = {
  available: boolean;
  latencyMs: number;
  statusCode: number | null;
  error: string | null;
};

export type HistoryMessage = {
  role: string;
  content: string;
};

export type HistorySessionSummary = {
  sessionId: string;
  provider: string;
  title: string;
  timestamp: number | null;
  path: string;
  messageCount: number;
};

export type HistoryProviderGroup = {
  provider: string;
  sessions: HistorySessionSummary[];
};

export type HistoryConversation = {
  sessionId: string;
  provider: string;
  title: string;
  timestamp: number | null;
  path: string;
  messages: HistoryMessage[];
};

export type DeleteHistoryResponse = {
  successCount: number;
  failureCount: number;
  errors: string[];
};
