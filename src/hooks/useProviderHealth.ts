import { useEffect, useRef, useState } from "react";
import { checkProviderHealth } from "../lib/api";
import type { HealthCheckResponse, ProviderView } from "../lib/types";

type HealthStatus = HealthCheckResponse | "loading";

type UseProviderHealthOptions = {
  setMessage: (message: string) => void;
};

export function useProviderHealth({ setMessage }: UseProviderHealthOptions) {
  const [healthCheckResults, setHealthCheckResults] = useState<Record<string, HealthStatus>>({});
  const healthTimersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  useEffect(() => {
    return () => {
      for (const timer of healthTimersRef.current.values()) {
        clearTimeout(timer);
      }
      healthTimersRef.current.clear();
    };
  }, []);

  async function healthCheckProvider(provider: ProviderView) {
    const baseUrl = (provider.baseUrl || "").trim();
    if (!baseUrl) {
      setMessage("该配置没有 Base URL。");
      return;
    }

    setHealthCheckResults((prev) => ({ ...prev, [provider.id]: "loading" }));

    const existingTimer = healthTimersRef.current.get(provider.id);
    if (existingTimer) {
      clearTimeout(existingTimer);
      healthTimersRef.current.delete(provider.id);
    }

    try {
      const result = await checkProviderHealth(baseUrl, provider.id);
      setHealthCheckResults((prev) => ({ ...prev, [provider.id]: result }));

      if (result.available) {
        setMessage(`${provider.name || provider.id} 可用，延迟 ${result.latencyMs}ms。`);
      } else {
        const detail = result.error ? `: ${result.error}` : "";
        setMessage(`${provider.name || provider.id} 不可用${detail}`);
      }

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

  return { healthCheckResults, healthCheckProvider };
}
