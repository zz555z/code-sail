import { useCallback, useRef, useState } from "react";
import type { ProviderDraft } from "../lib/types";

export type ModelTarget = "main" | "haiku" | "opus" | "sonnet";

type ModelField = "model" | "claudeHaikuModel" | "claudeOpusModel" | "claudeSonnetModel";

const MODEL_TARGET_TO_FIELD: Record<ModelTarget, ModelField> = {
  main: "model",
  haiku: "claudeHaikuModel",
  opus: "claudeOpusModel",
  sonnet: "claudeSonnetModel"
};

type UseModelSelectionOptions = {
  draft: ProviderDraft;
  draftRef: React.MutableRefObject<ProviderDraft>;
  setDraft: React.Dispatch<React.SetStateAction<ProviderDraft>>;
};

export function useModelSelection({ draft, draftRef, setDraft }: UseModelSelectionOptions) {
  const [modelValue, setModelValueState] = useState("");
  const [claudeHaikuModel, setClaudeHaikuModelState] = useState("");
  const [claudeOpusModel, setClaudeOpusModelState] = useState("");
  const [claudeSonnetModel, setClaudeSonnetModelState] = useState("");
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const [modelMenuTarget, setModelMenuTarget] = useState<ModelTarget>("main");

  const modelValueRef = useRef("");
  const claudeHaikuModelRef = useRef("");
  const claudeOpusModelRef = useRef("");
  const claudeSonnetModelRef = useRef("");

  const updateModelValue = useCallback((model: string) => {
    modelValueRef.current = model;
    setModelValueState(model);
  }, []);

  const setClaudeModel = useCallback(
    (target: Exclude<ModelTarget, "main">, value: string) => {
      const refMap = {
        haiku: claudeHaikuModelRef,
        opus: claudeOpusModelRef,
        sonnet: claudeSonnetModelRef
      };
      const setterMap = {
        haiku: setClaudeHaikuModelState,
        opus: setClaudeOpusModelState,
        sonnet: setClaudeSonnetModelState
      };
      const fieldMap: Record<Exclude<ModelTarget, "main">, ModelField> = {
        haiku: "claudeHaikuModel",
        opus: "claudeOpusModel",
        sonnet: "claudeSonnetModel"
      };

      refMap[target].current = value;
      setterMap[target](value);
      setDraft((current) => {
        const next = { ...current, [fieldMap[target]]: value };
        draftRef.current = next;
        return next;
      });
    },
    [setDraft, draftRef]
  );

  const syncModelRefs = useCallback(
    (newDraft: ProviderDraft, newModelValue: string) => {
      modelValueRef.current = newModelValue;
      claudeHaikuModelRef.current = newDraft.claudeHaikuModel;
      claudeOpusModelRef.current = newDraft.claudeOpusModel;
      claudeSonnetModelRef.current = newDraft.claudeSonnetModel;
      setModelValueState(newModelValue);
      setClaudeHaikuModelState(newDraft.claudeHaikuModel);
      setClaudeOpusModelState(newDraft.claudeOpusModel);
      setClaudeSonnetModelState(newDraft.claudeSonnetModel);
    },
    []
  );

  const selectModel = useCallback(
    (model: string, target: ModelTarget = "main") => {
      if (target === "main") {
        updateModelValue(model);
        setDraft((current) => {
          const next = { ...current, model };
          draftRef.current = next;
          return next;
        });
      } else {
        setClaudeModel(target, model);
      }
      setModelMenuOpen(false);
    },
    [updateModelValue, setClaudeModel, setDraft, draftRef]
  );

  const openModelMenu = useCallback(
    (target: ModelTarget, modelsCount: number) => {
      setModelMenuTarget(target);
      setModelMenuOpen(modelsCount > 0);
    },
    []
  );

  const resetModelStates = useCallback(() => {
    modelValueRef.current = "";
    claudeHaikuModelRef.current = "";
    claudeOpusModelRef.current = "";
    claudeSonnetModelRef.current = "";
    setModelValueState("");
    setClaudeHaikuModelState("");
    setClaudeOpusModelState("");
    setClaudeSonnetModelState("");
    setModelMenuOpen(false);
  }, []);

  const setClaudeHaikuModel = useCallback((value: string) => setClaudeModel("haiku", value), [setClaudeModel]);
  const setClaudeOpusModel = useCallback((value: string) => setClaudeModel("opus", value), [setClaudeModel]);
  const setClaudeSonnetModel = useCallback((value: string) => setClaudeModel("sonnet", value), [setClaudeModel]);

  return {
    modelValue,
    claudeHaikuModel,
    claudeOpusModel,
    claudeSonnetModel,
    modelMenuOpen,
    modelMenuTarget,
    modelValueRef,
    claudeHaikuModelRef,
    claudeOpusModelRef,
    claudeSonnetModelRef,
    setModelValue: updateModelValue,
    setClaudeHaikuModel,
    setClaudeOpusModel,
    setClaudeSonnetModel,
    selectModel,
    openModelMenu,
    setModelMenuOpen,
    setModelMenuTarget,
    syncModelRefs,
    resetModelStates
  };
}
