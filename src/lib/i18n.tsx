import {
  createContext,
  ReactNode,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { setAppLocale } from "./ipc";
import { initialLocale, Locale, persistLocale } from "./locale";
import { translateText } from "./translations.zh-CN";

export { translateText } from "./translations.zh-CN";
export type { Locale } from "./locale";

export function localize(locale: Locale, value: string): string {
  return locale === "zh-CN" ? translateText(value) : value;
}

interface LocaleContextValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (value: string) => string;
}

const LocaleContext = createContext<LocaleContextValue | null>(null);

export function LocaleProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(initialLocale);
  const value = useMemo(() => ({
    locale,
    t(value: string) {
      return localize(locale, value);
    },
    setLocale(next: Locale) {
      persistLocale(next);
      setLocaleState(next);
    },
  }), [locale]);

  useEffect(() => {
    document.documentElement.lang = locale;
    void setAppLocale(locale).catch(() => {
      // Browser preview does not expose Tauri IPC.
    });
  }, [locale]);

  return <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>;
}

export function useLocale(): LocaleContextValue {
  const context = useContext(LocaleContext);
  if (!context) throw new Error("useLocale must be used inside LocaleProvider");
  return context;
}
