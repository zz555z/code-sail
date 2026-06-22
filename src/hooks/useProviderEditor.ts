import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  copyProvider as copyProviderCommand,
  deleteProvider,
  fetchModels,
  getAppState,
  getProviderDetail,
  importCodexProvidersToClaude,
  reorderProviders as reorderProvidersCommand,
  restartCodexApp,
  refreshTrayMenu,
  saveProvider,
  setCurrentModel as setCurrentModelCommand
} from "../lib/api";
import { comparableDraft, draftFromProvider, emptyDraft } from "../lib/providerDraft";
import type { AppState, ProviderDraft, ProviderView } from "../lib/types";
import { errorMessage } from "../lib/utils";
import { useProviderHealth } from "./useProviderHealth";
import { useStateWithRef } from "./useStateWithRef";

type UseProviderEditorOptions = {
  setMessage: (message: string) => void;
  setMessagePaused: (paused: boolean) => void;
};

export function useProviderEditor({ setMessage, setMessagePaused }: UseProviderEditorOptions) {
  const [state, setState, stateRef] = useStateWithRef<AppState | null>(null);
  const [selectedId, setSelectedId, selectedIdRef] = useStateWithRef<string | null>(null);
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
  const [updateConfigFile, setUpdateConfigFile, updateConfigFileRef] = useStateWithRef(true);
  const draftRef = useRef<ProviderDraft>({ ...emptyDraft });
  const cleanDraftRef = useRef<ProviderDraft>({ ...emptyDraft });
  const modelValueRef = useRef("");
  const modelComboboxRef = useRef<HTMLDivElement>(null);
  const selectedRef = useRef<ProviderView | null>(null);
  const { healthCheckResults, healthCheckProvider } = useProviderHealth({ setMessage });

  const syncTrayMenu = useCallback(() => {
    void refreshTrayMenu().catch((error) => {
      console.warn("Failed to refresh tray menu:", errorMessage(error));
    });
  }, []);

  const refresh = useCallback(async (options?: { preferredId?: string | null }) => {
    let next: AppState;
    try {
      next = await getAppState();
    } catch (error) {
      setMessage(errorMessage(error));
      return;
    }
    setState(next);

    const hasPreferredId = Object.prototype.hasOwnProperty.call(options || {}, "preferredId");
    const desiredId = hasPreferredId
      ? options?.preferredId
      : selectedIdRef.current ?? next.activeProvider ?? next.providers[0]?.id ?? null;
    const nextSelected =
      desiredId && next.providers.some((provider) => provider.id === desiredId)
        ? desiredId
        : next.activeProvider ?? next.providers[0]?.id ?? null;

    setSelectedId(nextSelected);
    const provider = next.providers.find((item) => item.id === nextSelected) || null;
    const nextDraft = draftFromProvider(provider);
    const nextModelValue = provider?.model || (provider?.id === next.activeProvider ? next.activeModel ?? "" : "");
    draftRef.current = nextDraft;
    cleanDraftRef.current = nextDraft;
    modelValueRef.current = nextModelValue;
    setDraft(nextDraft);
    setModels(provider?.models ?? []);
    setModelValue(nextModelValue);
  }, [setMessage]);

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
    const found = state.providers.find((provider) => provider.id === selectedId) || null;
    selectedRef.current = found;
    return found;
  }, [selectedId, state]);

  const providerCount = state?.providers.length ?? 0;
  const activeProvider = useMemo(
    () => state?.providers.find((provider) => provider.id === state.activeProvider) || null,
    [state]
  );

  const isDirty = useMemo(() => {
    const current = comparableDraft(draft);
    const clean = comparableDraft(cleanDraftRef.current);
    return current.name !== clean.name || current.baseUrl !== clean.baseUrl || current.model !== clean.model || current.token !== clean.token;
  }, [draft]);

  const canSave = isDirty || updateConfigFile;

  const updateDraft = useCallback((patch: Partial<ProviderDraft>) => {
    setDraft((current) => {
      const next = { ...current, ...patch };
      draftRef.current = next;
      return next;
    });
  }, []);

  const updateModelValue = useCallback((model: string) => {
    modelValueRef.current = model;
    setModelValue(model);
  }, []);

  const selectModel = useCallback((model: string) => {
    updateModelValue(model);
    setDraft((current) => {
      const next = { ...current, model };
      draftRef.current = next;
      return next;
    });
    setModelMenuOpen(false);
    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur();
    }
  }, [updateModelValue]);

  const openCreateProvider = useCallback(() => {
    const nextDraft = { ...emptyDraft, toolType: stateRef.current?.activeTool ?? "codex" };
    setSelectedId(null);
    draftRef.current = nextDraft;
    cleanDraftRef.current = nextDraft;
    modelValueRef.current = "";
    setDraft(nextDraft);
    setModels([]);
    setModelMenuOpen(false);
    setModelValue("");
    setTokenVisible(false);
    setMessage("");
    setEditorOpen(true);
  }, [setMessage]);

  const openEditProvider = useCallback(async (provider: ProviderView) => {
    setBusy(true);
    setMessage("");
    try {
      const detail = await getProviderDetail(provider.id);
      const nextDraft = draftFromProvider(detail);
      const currentState = stateRef.current;
      const nextModelValue = detail.model || (detail.id === currentState?.activeProvider ? currentState?.activeModel ?? "" : "");
      setSelectedId(detail.id);
      draftRef.current = nextDraft;
      cleanDraftRef.current = nextDraft;
      modelValueRef.current = nextModelValue;
      setDraft(nextDraft);
      setModels(detail.models ?? []);
      setModelMenuOpen(false);
      setModelValue(nextModelValue);
      setTokenVisible(false);
      setEditorOpen(true);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }, [setMessage]);

  const closeEditor = useCallback(() => {
    setEditorOpen(false);
    setModels([]);
    setModelMenuOpen(false);
    setTokenVisible(false);
    setMessage("");
  }, [setMessage]);

  const copyProvider = useCallback(async (providerId: string) => {
    setBusy(true);
    setMessage("");
    try {
      const result = await copyProviderCommand(providerId);
      setModels([]);
      setModelMenuOpen(false);
      modelValueRef.current = "";
      setModelValue("");
      setEditorOpen(false);
      await refresh({ preferredId: result.providerId });
      syncTrayMenu();
      setMessage(`已复制为 ${result.providerId}。`);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }, [setMessage, refresh, syncTrayMenu]);

  const importFromCodexToClaude = useCallback(async () => {
    setImportingProviders(true);
    setBusy(true);
    setMessage("");
    try {
      const result = await importCodexProvidersToClaude();
      await refresh({ preferredId: null });
      syncTrayMenu();
      setMessage(
        result.importedCount > 0
          ? `已导入 ${result.importedCount} 条 codex 配置到 Claude。`
          : "没有可导入的 codex 配置。"
      );
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setImportingProviders(false);
      setBusy(false);
    }
  }, [setMessage, refresh, syncTrayMenu]);

  const removeProvider = useCallback(async (providerId: string) => {
    setBusy(true);
    setMessage("");
    try {
      await deleteProvider(providerId);
      setModels([]);
      setModelMenuOpen(false);
      if (selectedIdRef.current === providerId) {
        setEditorOpen(false);
        modelValueRef.current = "";
        setModelValue("");
      }
      await refresh({ preferredId: null });
      syncTrayMenu();
      setMessage(`已删除 ${providerId}。`);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }, [setMessage, refresh, syncTrayMenu]);

  const reorderProviders = useCallback(async (providerIds: string[]) => {
    const currentState = stateRef.current;
    if (!currentState) return;

    const currentIds = currentState.providers.map((provider) => provider.id);
    const sameOrder =
      providerIds.length === currentIds.length &&
      providerIds.every((providerId, index) => providerId === currentIds[index]);
    if (sameOrder) return;

    const providerMap = new Map(currentState.providers.map((provider) => [provider.id, provider]));
    const nextProviders = providerIds
      .map((providerId) => providerMap.get(providerId))
      .filter((provider): provider is ProviderView => Boolean(provider));

    if (nextProviders.length !== currentState.providers.length) {
      setMessage("配置列表已变化，请刷新后再排序。");
      return;
    }

    const previousState = currentState;
    setBusy(true);
    setMessage("");
    setState({ ...currentState, providers: nextProviders });

    try {
      await reorderProvidersCommand(providerIds);
      syncTrayMenu();
      setMessage("已更新配置顺序。");
    } catch (error) {
      setState(previousState);
      await refresh({ preferredId: selectedIdRef.current });
      setMessage(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }, [setMessage, refresh, syncTrayMenu]);

  const setCurrentProvider = useCallback(async (provider: ProviderView) => {
    const model = (provider.model || "").trim();
    if (!model) {
      setMessage("请先为该配置填写 Model。");
      return;
    }
    const shouldUpdateConfig = updateConfigFileRef.current;
    if (shouldUpdateConfig && !provider.tokenPresent) {
      setMessage("请先为该配置填写 Token。");
      return;
    }

    setBusy(true);
    setMessage("");
    try {
      await setCurrentModelCommand(provider.id, model, "", shouldUpdateConfig);
      setEditorOpen(false);
      setModels([]);
      setModelMenuOpen(false);
      setSelectedId(provider.id);
      setState((current) =>
        current
          ? {
              ...current,
              activeProvider: provider.id,
              activeModel: model,
              providers: current.providers.map((item) =>
                item.id === provider.id ? { ...item, model } : item
              )
            }
          : current
      );
      syncTrayMenu();
      setMessage(
        shouldUpdateConfig
          ? `已设置 ${provider.name || provider.id} 为当前模型，并更新配置文件。`
          : `已设置 ${provider.name || provider.id} 为当前模型。`
      );
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }, [setMessage, syncTrayMenu]);

  const saveCurrentProvider = useCallback(async () => {
    const model = modelValueRef.current.trim();
    const draftToSave: ProviderDraft = { ...draftRef.current, model };
    const baseUrl = draftToSave.baseUrl.trim();
    const shouldUpdateConfig = updateConfigFileRef.current;

    if (!baseUrl) {
      setMessage("请先填写 Base URL。");
      return;
    }
    if (shouldUpdateConfig && !model) {
      setMessage("请先选择 Model，或关闭更新配置文件。");
      return;
    }
    if (shouldUpdateConfig && !draftToSave.token.trim()) {
      setMessage("请先填写 Token，或关闭更新配置文件。");
      return;
    }

    setBusy(true);
    setMessage("");

    let savedProvider = false;
    try {
      const result = await saveProvider(draftToSave, shouldUpdateConfig);
      savedProvider = true;

      await refresh({ preferredId: result.providerId });
      setEditorOpen(false);
      setModels([]);
      setModelMenuOpen(false);
      syncTrayMenu();
      setMessage(shouldUpdateConfig && model ? "已保存配置并写入当前模型。" : "已保存配置。");
    } catch (error) {
      const detail = errorMessage(error);
      if (savedProvider) {
        await refresh({ preferredId: draftRef.current.originalId || null });
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
  }, [setMessage, refresh, syncTrayMenu]);

  const fetchProviderModels = useCallback(async () => {
    const requestDraft = { ...draftRef.current, model: modelValueRef.current.trim() };
    const baseUrl = requestDraft.baseUrl.trim();
    const token = requestDraft.token.trim();
    const currentSelected = selectedRef.current;
    const canUseSavedToken = Boolean(currentSelected?.tokenPresent && currentSelected.id === requestDraft.originalId);
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
      const result = await fetchModels(requestDraft);
      const nextModel = requestDraft.model.trim();
      const fetchedProviderId = result.providerId || requestDraft.originalId || null;
      const currentState = stateRef.current;
      const knownProvider = Boolean(
        fetchedProviderId && currentState?.providers.some((provider) => provider.id === fetchedProviderId)
      );
      setModels(result.models);
      if (fetchedProviderId) {
        setSelectedId(fetchedProviderId);
      }
      setDraft((current) => ({
        ...current,
        originalId: fetchedProviderId || current.originalId,
        model: nextModel
      }));
      draftRef.current = {
        ...draftRef.current,
        originalId: fetchedProviderId || draftRef.current.originalId,
        model: nextModel
      };
      updateModelValue(nextModel);
      setModelMenuOpen(result.models.length > 0);
      if (knownProvider) {
        setState((current) =>
          current
            ? {
                ...current,
                providers: current.providers.map((provider) =>
                  provider.id === fetchedProviderId
                    ? { ...provider, models: result.models }
                    : provider
                )
              }
            : current
        );
      } else if (fetchedProviderId) {
        await refresh({ preferredId: fetchedProviderId });
      }
      setMessage(
        fetchedProviderId
          ? `已获取并保存 ${result.models.length} 个模型，可在下拉框选择。`
          : `已获取 ${result.models.length} 个模型，可在下拉框选择。`
      );
    } catch (error) {
      setModels([]);
      setModelMenuOpen(false);
      setMessage(`${errorMessage(error)}。也可以手动填写模型名称。`);
    } finally {
      setLoadingModels(false);
      setMessagePaused(false);
    }
  }, [setMessage, setMessagePaused, refresh, updateModelValue]);

  const restartCodex = useCallback(async () => {
    setRestarting(true);
    setMessage("正在重启 Codex...");
    try {
      await restartCodexApp();
      setMessage("已请求重启 Codex。");
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setRestarting(false);
    }
  }, [setMessage]);

  const toggleTokenVisible = useCallback(() => {
    setTokenVisible((visible) => !visible);
  }, []);

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
    setModelValue: updateModelValue,
    toggleTokenVisible,
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
