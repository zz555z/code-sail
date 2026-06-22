import { useCallback, useState } from "react";
import { getToolStatuses, openClaudeTerminal, openCodexTerminal, openToolInstall } from "../lib/api";
import type { ToolStatus, ToolType } from "../lib/types";
import { errorMessage } from "../lib/utils";

type UseToolStatusesOptions = {
  setMessage: (message: string) => void;
};

export function useToolStatuses({ setMessage }: UseToolStatusesOptions) {
  const [toolStatuses, setToolStatuses] = useState<ToolStatus[]>([]);
  const [toolStatusesLoading, setToolStatusesLoading] = useState(false);
  const [openingTerminal, setOpeningTerminal] = useState(false);
  const [installingTool, setInstallingTool] = useState<string | null>(null);

  const refreshToolStatuses = useCallback(async () => {
    setToolStatusesLoading(true);
    try {
      setToolStatuses(await getToolStatuses());
    } catch (error) {
      setToolStatuses([]);
      setMessage(errorMessage(error));
    } finally {
      setToolStatusesLoading(false);
    }
  }, [setMessage]);

  const openInTerminal = useCallback(async (toolType: ToolType) => {
    setOpeningTerminal(true);
    setMessage("");
    const toolName = toolType === "claude" ? "Claude" : "Codex";
    try {
      if (toolType === "claude") {
        await openClaudeTerminal();
      } else {
        await openCodexTerminal();
      }
      setMessage(`已打开终端启动 ${toolName}。`);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setOpeningTerminal(false);
    }
  }, [setMessage]);

  const openToolInstaller = useCallback(async (tool: ToolStatus) => {
    setInstallingTool(tool.command);
    setMessage("");
    try {
      await openToolInstall(tool.command);
      setMessage(`已打开 ${tool.name} 安装页面。`);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setInstallingTool(null);
    }
  }, [setMessage]);

  return {
    toolStatuses,
    toolStatusesLoading,
    openingTerminal,
    installingTool,
    refreshToolStatuses,
    openInTerminal,
    openToolInstaller
  };
}
