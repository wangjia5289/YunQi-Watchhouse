export type Locale = "en" | "zh-CN";

const STORAGE_KEY = "watchhouse.locale";

export function initialLocale(): Locale {
  const stored = window.localStorage.getItem(STORAGE_KEY);
  if (stored === "en" || stored === "zh-CN") return stored;
  return navigator.language.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}

export function persistLocale(locale: Locale): void {
  window.localStorage.setItem(STORAGE_KEY, locale);
}
