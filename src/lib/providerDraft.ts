import type { ProviderDraft, ProviderView } from "./types";

export const emptyDraft: ProviderDraft = {
  originalId: null,
  name: "",
  baseUrl: "",
  model: "",
  token: ""
};

export function draftFromProvider(provider: ProviderView | null): ProviderDraft {
  if (!provider) return { ...emptyDraft };
  return {
    originalId: provider.id,
    name: provider.name || provider.id,
    baseUrl: provider.baseUrl || "",
    model: provider.model || "",
    token: provider.token || ""
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

export function comparableProvider(provider: ProviderView) {
  return {
    name: provider.name || provider.id,
    baseUrl: provider.baseUrl || "",
    model: provider.model || "",
    token: provider.token || ""
  };
}
