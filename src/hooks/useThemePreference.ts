import { useEffect, useState } from "react";
import {
  nextThemePreference,
  persistThemePreference,
  resolveTheme,
  storedThemePreference
} from "../lib/theme";

export function useThemePreference() {
  const [themePreference, setThemePreference] = useState(storedThemePreference);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");

    function applyTheme() {
      document.documentElement.dataset.theme = resolveTheme(themePreference);
      document.documentElement.dataset.themePreference = themePreference;
    }

    applyTheme();
    persistThemePreference(themePreference);
    media.addEventListener("change", applyTheme);
    return () => media.removeEventListener("change", applyTheme);
  }, [themePreference]);

  return {
    themePreference,
    cycleTheme: () => setThemePreference((current) => nextThemePreference(current))
  };
}
