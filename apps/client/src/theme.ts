export type Theme = "system" | "light" | "dark";
const key = "honkoku.theme";
export function savedTheme(): Theme {
  try {
    const value = localStorage.getItem(key);
    return value === "light" || value === "dark" ? value : "system";
  } catch {
    return "system";
  }
}
function apply(theme: Theme) {
  if (theme === "system") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
  document.documentElement.dataset.themeReady = "";
}
export function setTheme(theme: Theme) {
  apply(theme);
  try {
    localStorage.setItem(key, theme);
  } catch {
    /* The selection still applies for this session. */
  }
}
apply(savedTheme());
