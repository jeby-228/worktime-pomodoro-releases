export type ThemeChoice = "light" | "dark" | "system";

const RESOLVED_KEY = "worktime-theme";

export function resolveTheme(theme: ThemeChoice): "light" | "dark" {
  if (theme === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return theme;
}

export function applyTheme(theme: ThemeChoice): boolean {
  const resolved = resolveTheme(theme);
  const root = document.documentElement;
  root.setAttribute("data-theme", resolved);
  root.classList.toggle("dark", resolved === "dark");
  root.style.colorScheme = resolved;
  try {
    localStorage.setItem(RESOLVED_KEY, resolved);
  } catch {
    // Private mode can block storage; the current page is already themed.
  }
  return resolved === "dark";
}
