import { useEffect, useMemo, useRef, useState } from "react";
import {
  checkProviderHealth,
  copyProvider as copyProviderCommand,
  deleteProvider,
  fetchModels,
  getAppState,
  importCodexProvidersToClaude,
  reorderProviders as reorderProvidersCommand,
  restartCodexApp,
  saveProvider,
  setCurrentModel as setCurrentModelCommand
} from "../lib/api";
import { comparableDraft, comparableProvider, draftFromProvider, emptyDraft } from "../lib/providerDraft";
import type { AppState, HealthCheckResponse, ProviderDraft, ProviderView } from "../lib/types";

type HealthStatus = HealthCheckResponse | "loading";

type UseProviderEditorOptions = {
  setMessage: (message: string) => void;
  setMessagePaused: (paused: boolean) => void;
};

export function useProviderEditor({ setMessage, setMessagePaused }: UseProviderEditorOptions) {
  const [state, setState] = useState<AppState | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<ProviderDraft>({ ...emptyDraft });
  const [models, setModels] = useState<string[]>([]);
  const [modelValue, setModelValue] = useState("");
  const [editorOpen, setEditorOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [importingProviders, setImportingProviders] = useState(false);
  const [restarting, setRestarting] = useState(false);
  const [loadingModels, setLoadingModels] = useState(false);
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const [tokenVisible, setTokenVisible] = useState(false);
  const [updateConfigFile, setUpdateConfigFile] = useState(true);
  const [healthCheckResults, setHealthCheckResults] = useState<Record<string, HealthStatus>>({});
  const healthTimersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());
  const modelComboboxRef = useRef<HTMLDivElement>(null);

  async function refresh(options?: { preferredId?: string | null }) {
    let next: AppState;
    try {
      next = await getAppState();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
      return;
    }
    setState(next);

    const hasPreferredId = Object.prototype.hasOwnProperty.call(options || {}, "preferredId");
    const desiredId = hasPreferredId
      ? options?.preferredId
      : selectedId ?? next.activeProvider ?? next.providers[0]?.id ?? null;
    const nextSelected =
      desiredId && next.providers.some((provider) => provider.id === desiredId)
        ? desiredId
        : next.activeProvider ?? next.providers[0]?.id ?? null;

    setSelectedId(nextSelected);
    const provider = next.providers.find((item) => item.id === nextSelected) || null;
    setDraft(draftFromProvider(provider));
    setModels(provider?.models ?? []);
    setModelValue(provider?.model || (provider?.id === next.activeProvider ? next.activeModel ?? "" : ""));
  }

  useEffect(() => {
    if (!modelMenuOpen) return;

    function handleOutsidePointerDown(event: PointerEvent) {
      const target = event.target;
      if (target instanceof Node && !modelComboboxRef.current?.contains(target)) {
        setModelMenuOpen(false);
      }
    }

    document.addEventListener("pointerdown", handleOutsidePointerDown);
    return () => document.removeEventListener("pointerdown", handleOutsidePointerDown);
  }, [modelMenuOpen]);

  const selected = useMemo(() => {
    if (!state) return null;
    return state.providers.find((provider) => provider.id === selectedId) || null;
  }, [selectedId, state]);

  const providerCount = state?.providers.length ?? 0;
  const activeProvider = useMemo(
    () => state?.providers.find((provider) => provider.id === state.activeProvider) || null,
    [state]
  );

  const isDirty = useMemo(() => {
    if (draft.token.trim()) return true;
    if (!selected) {
      const current = comparableDraft(draft);
      return Boolean(current.name || current.baseUrl || current.model);
    }
    return JSON.stringify(comparableDraft(draft)) !== JSON.stringify(comparableProvider(selected));
  }, [draft, selected]);

  const canSave = isDirty || updateConfigFile;

  function updateDraft(patch: Partial<ProviderDraft>) {
    setDraft((current) => ({ ...current, ...patch }));
  }

  function selectModel(model: string) {
    setModelValue(model);
    updateDraft({ model });
    setModelMenuOpen(false);
    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur();
    }
  }

  function openCreateProvider() {
    setSelectedId(null);
    setDraft({ ...emptyDraft, toolType: state?.activeTool ?? "codex" });
    setModels([]);
    setModelMenuOpen(false);
    setModelValue("");
    setTokenVisible(false);
    setMessage("");
    setEditorOpen(true);
  }

  function openEditProvider(provider: ProviderView) {
    setSelectedId(provider.id);
    setDraft(draftFromProvider(provider));
    setModels(provider.models ?? []);
    setModelMenuOpen(false);
    setModelValue(provider.model || (provider.id === state?.activeProvider ? state?.activeModel ?? "" : ""));
    setTokenVisible(false);
    setMessage("");
    setEditorOpen(true);
  }

  function closeEditor() {
    setEditorOpen(false);
    setModels([]);
    setModelMenuOpen(false);
    setTokenVisible(false);
    setMessage("");
  }

  async function copyProvider(providerId: string) {
    setBusy(true);
    setMessage("");
    try {
      const result = await copyProviderCommand(providerId);
      setModels([]);
      setModelMenuOpen(false);
      setModelValue("");
      setEditorOpen(false);
      await refresh({ preferredId: result.providerId });
      setMessage(`已复制为 ${result.providerId}。`);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function importFromCodexToClaude() {
    setImportingProviders(true);
    setBusy(true);
    setMessage("");
    try {
      const result = await importCodexProvidersToClaude();
      await refresh({ preferredId: null });
      setMessage(
        result.importedCount > 0
          ? `已导入 ${result.importedCount} 条 codex 配置到 Claude。`
          : "没有可导入的 codex 配置。"
      );
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setImportingProviders(false);
      setBusy(false);
    }
  }

  async function removeProvider(providerId: string) {
    setBusy(true);
    setMessage("");
    try {
      await deleteProvider(providerId);
      setModels([]);
      setModelMenuOpen(false);
      if (selectedId === providerId) {
        setEditorOpen(false);
        setModelValue("");
      }
      await refresh({ preferredId: null });
      setMessage(`已删除 ${providerId}。`);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function reorderProviders(providerIds: string[]) {
    if (!state) return;

    const currentIds = state.providers.map((provider) => provider.id);
    const sameOrder =
      providerIds.length === currentIds.length &&
      providerIds.every((providerId, index) => providerId === currentIds[index]);
    if (sameOrder) return;

    const providerMap = new Map(state.providers.map((provider) => [provider.id, provider]));
    const nextProviders = providerIds
      .map((providerId) => providerMap.get(providerId))
      .filter((provider): provider is ProviderView => Boolean(provider));

    if (nextProviders.length !== state.providers.length) {
      setMessage("配置列表已变化，请刷新后再排序。");
      return;
    }

    const previousState = state;
    setBusy(true);
    setMessage("");
    setState({ ...state, providers: nextProviders });

    try {
      await reorderProvidersCommand(providerIds);
      setMessage("已更新配置顺序。");
    } catch (error) {
      setState(previousState);
      await refresh({ preferredId: selectedId });
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function setCurrentProvider(provider: ProviderView) {
    const model = (provider.model || "").trim();
    if (!model) {
      setMessage("请先为该配置填写 Model。");
      return;
    }
    if (updateConfigFile && !provider.tokenPresent && !provider.token?.trim()) {
      setMessage("请先为该配置填写 Token。");
      return;
    }

    setBusy(true);
    setMessage("");
    try {
      await setCurrentModelCommand(provider.id, model, provider.token || "", updateConfigFile);
      setEditorOpen(false);
      setModels([]);
      setModelMenuOpen(false);
      await refresh({ preferredId: provider.id });
      setMessage(
        updateConfigFile
          ? `已设置 ${provider.name || provider.id} 为当前模型，并更新配置文件。`
          : `已设置 ${provider.name || provider.id} 为当前模型。`
      );
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function saveCurrentProvider() {
    const baseUrl = draft.baseUrl.trim();
    const model = modelValue.trim();
    const draftToSave: ProviderDraft = { ...draft, model };

    if (!baseUrl) {
      setMessage("请先填写 Base URL。");
      return;
    }
    if (updateConfigFile && !model) {
      setMessage("请先选择 Model，或关闭更新配置文件。");
      return;
    }
    if (updateConfigFile && !draftToSave.token.trim()) {
      setMessage("请先填写 Token，或关闭更新配置文件。");
      return;
    }

    setBusy(true);
    setMessage("");

    let savedProvider = false;
    try {
      const result = await saveProvider(draftToSave, updateConfigFile);
      savedProvider = true;

      await refresh({ preferredId: result.providerId });
      setEditorOpen(false);
      setModels([]);
      setModelMenuOpen(false);
      setMessage(updateConfigFile && model ? "已保存配置并写入当前模型。" : "已保存配置。");
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      if (savedProvider) {
        await refresh({ preferredId: draft.originalId || null });
        setEditorOpen(false);
        setModels([]);
        setModelMenuOpen(false);
        setMessage(`配置已保存，已回到列表；但后续处理失败：${detail}`);
      } else {
        setMessage(detail);
      }
    } finally {
      setBusy(false);
    }
  }

  async function fetchProviderModels() {
    const baseUrl = draft.baseUrl.trim();
    const token = draft.token.trim();
    const canUseSavedToken = Boolean(selected?.tokenPresent && selected.id === draft.originalId);
    if (!baseUrl) {
      setMessage("请先填写 Base URL。");
      return;
    }
    if (!token && !canUseSavedToken) {
      setMessage("请先填写 Token，或选择一个已经保存 token 的配置。");
      return;
    }

    setLoadingModels(true);
    setMessagePaused(true);
    setMessage("正在请求模型列表...");
    try {
      const result = await fetchModels({ ...draft, model: modelValue.trim() });
      const nextModel = modelValue.trim() || result.models[0] || "";
      setModels(result.models);
      setSelectedId(result.providerId);
      setDraft((current) => ({ ...current, originalId: result.providerId, model: nextModel }));
      setModelValue(nextModel);
      setModelMenuOpen(result.models.length > 0);
      setState(await getAppState());
      setMessage(`已获取并保存 ${result.models.length} 个模型，可在下拉框选择。`);
    } catch (error) {
      setModels([]);
      setModelMenuOpen(false);
      setMessage(`${error instanceof Error ? error.message : String(error)}。也可以手动填写模型名称。`);
    } finally {
      setLoadingModels(false);
      setMessagePaused(false);
    }
  }

  async function restartCodex() {
    setRestarting(true);
    setMessage("正在重启 Codex...");
    try {
      await restartCodexApp();
      setMessage("已请求重启 Codex。");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setRestarting(false);
    }
  }

  async function healthCheckProvider(provider: ProviderView) {
    const baseUrl = (provider.baseUrl || "").trim();
    if (!baseUrl) {
      setMessage("该配置没有 Base URL。");
      return;
    }

    setHealthCheckResults((prev) => ({ ...prev, [provider.id]: "loading" }));

    // Clear any existing auto-clear timer for this provider
    const existingTimer = healthTimersRef.current.get(provider.id);
    if (existingTimer) {
      clearTimeout(existingTimer);
      healthTimersRef.current.delete(provider.id);
    }

    try {
      const result = await checkProviderHealth(
        baseUrl,
        provider.token || "",
        provider.model || "",
        provider.toolType
      );
      setHealthCheckResults((prev) => ({ ...prev, [provider.id]: result }));

      if (result.available) {
        setMessage(`${provider.name || provider.id} 可用，延迟 ${result.latencyMs}ms。`);
      } else {
        const detail = result.error ? `: ${result.error}` : "";
        setMessage(`${provider.name || provider.id} 不可用${detail}`);
      }

      // Auto-clear result after 5 seconds
      const timer = setTimeout(() => {
        setHealthCheckResults((prev) => {
          const next = { ...prev };
          delete next[provider.id];
          return next;
        });
        healthTimersRef.current.delete(provider.id);
      }, 5000);
      healthTimersRef.current.set(provider.id, timer);
    } catch (error) {
      setHealthCheckResults((prev) => {
        const next = { ...prev };
        delete next[provider.id];
        return next;
      });
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  return {
    state,
    selected,
    selectedId,
    draft,
    models,
    modelValue,
    providerCount,
    activeProvider,
    editorOpen,
    busy,
    importingProviders,
    restarting,
    loadingModels,
    modelMenuOpen,
    tokenVisible,
    updateConfigFile,
    canSave,
    modelComboboxRef,
    setUpdateConfigFile,
    setModelMenuOpen,
    setModelValue,
    toggleTokenVisible: () => setTokenVisible((visible) => !visible),
    refresh,
    restartCodex,
    openCreateProvider,
    openEditProvider,
    copyProvider,
    importFromCodexToClaude,
    reorderProviders,
    setCurrentProvider,
    removeProvider,
    closeEditor,
    updateDraft,
    selectModel,
    fetchProviderModels,
    saveCurrentProvider,
    healthCheckResults,
    healthCheckProvider
  };
}
