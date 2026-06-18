import { useCallback, useState } from "react";
import { getActiveTool, setActiveTool } from "../lib/api";
import type { ToolType } from "../lib/types";

export function useActiveTool() {
  const [activeTool, setActiveToolState] = useState<ToolType>("codex");
  const [switching, setSwitching] = useState(false);

  const loadActiveTool = useCallback(async () => {
    try {
      const tool = await getActiveTool();
      setActiveToolState(tool);
    } catch {
      // Fall back to default
    }
  }, []);

  const switchTool = useCallback(async (tool: ToolType) => {
    if (tool === activeTool) return;
    setSwitching(true);
    try {
      await setActiveTool(tool);
      setActiveToolState(tool);
    } finally {
      setSwitching(false);
    }
  }, [activeTool]);

  return { activeTool, switching, loadActiveTool, switchTool };
}
