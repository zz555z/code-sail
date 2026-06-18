import { useEffect, useMemo, useRef, useState } from "react";
import {
  copyProvider as copyProviderCommand,
  deleteProvider,
  fetchModels,
  getAppState,
  getProviderDetail,
  importCodexProvidersToClaude,
  reorderProviders as reorderProvidersCommand,
  restartCodexApp,
  saveProvider,
  setCurrentModel as setCurrentModelCommand
} from "../lib/api";
import { comparableDraft, draftFromProvider, emptyDraft } from "../lib/providerDraft";
import type { AppState, ProviderDraft, ProviderView } from "../lib/types";
import { useProviderHealth } from "./useProviderHealth";

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
  const draftRef = useRef<ProviderDraft>({ ...emptyDraft });
  const cleanDraftRef = useRef<ProviderDraft>({ ...emptyDraft });
  const modelValueRef = useRef("");
  const modelComboboxRef = useRef<HTMLDivElement>(null);
  const { healthCheckResults, healthCheckProvider } = useProviderHealth({ setMessage });

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
    const nextDraft = draftFromProvider(provider);
    const nextModelValue = provider?.model || (provider?.id === next.activeProvider ? next.activeModel ?? "" : "");
    draftRef.current = nextDraft;
    cleanDraftRef.current = nextDraft;
    modelValueRef.current = nextModelValue;
    setDraft(nextDraft);
    setModels(provider?.models ?? []);
    setModelValue(nextModelValue);
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
    const current = comparableDraft(draft);
    return JSON.stringify(current) !== JSON.stringify(comparableDraft(cleanDraftRef.current));
  }, [draft]);

  const canSave = isDirty || updateConfigFile;

  function updateDraft(patch: Partial<ProviderDraft>) {
    setDraft((current) => {
      const next = { ...current, ...patch };
      draftRef.current = next;
      return next;
    });
  }

  function updateModelValue(model: string) {
    modelValueRef.current = model;
    setModelValue(model);
  }

  function selectModel(model: string) {
    updateModelValue(model);
    updateDraft({ model });
    setModelMenuOpen(false);
    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur();
    }
  }

  function openCreateProvider() {
    const nextDraft = { ...emptyDraft, toolType: state?.activeTool ?? "codex" };
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
  }

  async function openEditProvider(provider: ProviderView) {
    setBusy(true);
    setMessage("");
    try {
      const detail = await getProviderDetail(provider.id);
      const nextDraft = draftFromProvider(detail);
      const nextModelValue = detail.model || (detail.id === state?.activeProvider ? state?.activeModel ?? "" : "");
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
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
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
      updateModelValue("");
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
        updateModelValue("");
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
    if (updateConfigFile && !provider.tokenPresent) {
      setMessage("请先为该配置填写 Token。");
      return;
    }

    setBusy(true);
    setMessage("");
    try {
      await setCurrentModelCommand(provider.id, model, "", updateConfigFile);
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
    const model = modelValueRef.current.trim();
    const draftToSave: ProviderDraft = { ...draftRef.current, model };
    const baseUrl = draftToSave.baseUrl.trim();

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
  }

  async function fetchProviderModels() {
    const requestDraft = { ...draftRef.current, model: modelValueRef.current.trim() };
    const baseUrl = requestDraft.baseUrl.trim();
    const token = requestDraft.token.trim();
    const canUseSavedToken = Boolean(selected?.tokenPresent && selected.id === requestDraft.originalId);
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
      const knownProvider = Boolean(
        fetchedProviderId && state?.providers.some((provider) => provider.id === fetchedProviderId)
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
