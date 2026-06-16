import { useState } from "react";
import { getToolStatuses, openCodexTerminal, openToolInstall } from "../lib/api";
import type { ToolStatus } from "../lib/types";

type UseToolStatusesOptions = {
  setMessage: (message: string) => void;
};

export function useToolStatuses({ setMessage }: UseToolStatusesOptions) {
  const [toolStatuses, setToolStatuses] = useState<ToolStatus[]>([]);
  const [toolStatusesLoading, setToolStatusesLoading] = useState(false);
  const [openingCodexTerminal, setOpeningCodexTerminal] = useState(false);
  const [installingTool, setInstallingTool] = useState<string | null>(null);

  async function refreshToolStatuses() {
    setToolStatusesLoading(true);
    try {
      setToolStatuses(await getToolStatuses());
    } catch (error) {
      setToolStatuses([]);
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setToolStatusesLoading(false);
    }
  }

  async function openCodexInTerminal() {
    setOpeningCodexTerminal(true);
    setMessage("");
    try {
      await openCodexTerminal();
      setMessage("已打开终端启动 Codex。");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setOpeningCodexTerminal(false);
    }
  }

  async function openToolInstaller(tool: ToolStatus) {
    setInstallingTool(tool.command);
    setMessage("");
    try {
      await openToolInstall(tool.command);
      setMessage(`已打开 ${tool.name} 安装页面。`);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setInstallingTool(null);
    }
  }

  return {
    toolStatuses,
    toolStatusesLoading,
    openingCodexTerminal,
    installingTool,
    refreshToolStatuses,
    openCodexInTerminal,
    openToolInstaller
  };
}
