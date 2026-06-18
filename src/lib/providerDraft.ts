import type { ProviderDetail, ProviderDraft, ProviderView, ToolType } from "./types";

export const emptyDraft: ProviderDraft = {
  originalId: null,
  name: "",
  baseUrl: "",
  model: "",
  token: "",
  toolType: "codex"
};

export function draftFromProvider(provider: ProviderView | ProviderDetail | null): ProviderDraft {
  if (!provider) return { ...emptyDraft };
  return {
    originalId: provider.id,
    name: provider.name || provider.id,
    baseUrl: provider.baseUrl || "",
    model: provider.model || "",
    token: "token" in provider ? provider.token || "" : "",
    toolType: provider.toolType
  };
}

export function comparableDraft(draft: ProviderDraft) {
  return {
    name: draft.name.trim(),
    baseUrl: draft.baseUrl.trim(),
    model: draft.model.trim(),
    token: draft.token.trim()
  };
}
