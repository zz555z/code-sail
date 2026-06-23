import type { DeleteHistoryResponse } from "./types";

const historyTimeFormatter = new Intl.DateTimeFormat("zh-CN", {
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit"
});

export function formatHistoryTime(timestamp: number | null) {
  if (!timestamp) return "未知时间";
  const milliseconds = timestamp > 10_000_000_000 ? timestamp : timestamp * 1000;
  return historyTimeFormatter.format(new Date(milliseconds));
}

const ROLE_LABELS: Record<string, string> = {
  user: "User",
  assistant: "Assistant",
  system: "System",
  tool: "Tool"
};

function normalizeRole(role: string): string {
  return role.trim().toLowerCase();
}

export function roleLabel(role: string): string {
  return ROLE_LABELS[normalizeRole(role)] || role || "Unknown";
}

export function roleClass(role: string): string {
  const normalized = normalizeRole(role);
  return ROLE_LABELS[normalized] ? normalized : "other";
}

export function formatDeleteHistoryFailure(result: DeleteHistoryResponse) {
  const details = result.errors.map((item) => item.trim()).filter(Boolean).slice(0, 2);
  return details.length ? `删除失败 ${result.failureCount} 条：${details.join("；")}` : `删除失败 ${result.failureCount} 条。`;
}
