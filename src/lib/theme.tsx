import { Monitor, Moon, Sun } from "lucide-react";

export type ThemePreference = "system" | "light" | "dark";

const themeStorageKey = "codesail-theme";
const legacyThemeStorageKey = "codex-config-desktop-theme";

export function storedThemePreference(): ThemePreference {
  const value =
    window.localStorage.getItem(themeStorageKey) ?? window.localStorage.getItem(legacyThemeStorageKey);
  return value === "light" || value === "dark" || value === "system" ? value : "system";
}

export function resolveTheme(preference: ThemePreference) {
  if (preference !== "system") return preference;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function persistThemePreference(preference: ThemePreference) {
  window.localStorage.setItem(themeStorageKey, preference);
}

export function themeLabel(preference: ThemePreference) {
  if (preference === "system") return "跟随系统";
  if (preference === "light") return "浅色";
  return "深色";
}

export function nextThemePreference(preference: ThemePreference): ThemePreference {
  if (preference === "system") return "light";
  if (preference === "light") return "dark";
  return "system";
}

export function themeIcon(preference: ThemePreference) {
  if (preference === "system") return <Monitor size={17} />;
  if (preference === "light") return <Sun size={17} />;
  return <Moon size={17} />;
}
