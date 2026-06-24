import type { ProviderDetail, ProviderDraft, ProviderView, ToolType } from "./types";

export type ProviderTokenAction = "keep" | "replace" | "clear";

export const emptyDraft: ProviderDraft = {
  originalId: null,
  name: "",
  baseUrl: "",
  model: "",
  wireApi: "responses",
  requiresOpenaiAuth: false,
  token: "",
  toolType: "codex",
  claudeHaikuModel: "",
  claudeOpusModel: "",
  claudeSonnetModel: ""
};

export function draftFromProvider(provider: ProviderView | ProviderDetail | null): ProviderDraft {
  if (!provider) return { ...emptyDraft };
  return {
    originalId: provider.id,
    name: provider.name || provider.id,
    baseUrl: provider.baseUrl || "",
    model: provider.model || "",
    wireApi: provider.wireApi || "responses",
    requiresOpenaiAuth: provider.requiresOpenaiAuth ?? false,
    token: "token" in provider ? provider.token || "" : "",
    toolType: provider.toolType,
    claudeHaikuModel: provider.claudeHaikuModel || "",
    claudeOpusModel: provider.claudeOpusModel || "",
    claudeSonnetModel: provider.claudeSonnetModel || ""
  };
}

export function comparableDraft(draft: ProviderDraft) {
  return {
    name: draft.name.trim(),
    baseUrl: draft.baseUrl.trim(),
    model: draft.model.trim(),
    wireApi: draft.wireApi.trim(),
    requiresOpenaiAuth: draft.requiresOpenaiAuth,
    token: draft.token.trim(),
    claudeHaikuModel: draft.claudeHaikuModel.trim(),
    claudeOpusModel: draft.claudeOpusModel.trim(),
    claudeSonnetModel: draft.claudeSonnetModel.trim()
  };
}

export function draftsEqual(left: ProviderDraft, right: ProviderDraft): boolean {
  const current = comparableDraft(left);
  const clean = comparableDraft(right);
  return (Object.keys(current) as Array<keyof typeof current>).every(
    (key) => current[key] === clean[key]
  );
}

export function tokenActionFromDraft(draft: ProviderDraft, cleanDraft: ProviderDraft): ProviderTokenAction {
  const currentToken = draft.token.trim();
  const cleanToken = cleanDraft.token.trim();

  if (currentToken === cleanToken) return "keep";
  return currentToken ? "replace" : "clear";
}
