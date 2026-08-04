export const SETTINGS_TABS = [
  { id: "general", label: "General" },
  { id: "classification", label: "Classification" },
  { id: "focus-limits", label: "Focus & Limits" },
  { id: "data-safety", label: "Data & Safety" },
  { id: "diagnostics-updates", label: "Diagnostics & Updates" },
] as const;

export type SettingsTab = (typeof SETTINGS_TABS)[number]["id"];

export function nextSettingsTab(
  current: SettingsTab,
  direction: -1 | 1,
): SettingsTab {
  const currentIndex = SETTINGS_TABS.findIndex((tab) => tab.id === current);
  const nextIndex = (currentIndex + direction + SETTINGS_TABS.length) % SETTINGS_TABS.length;
  return SETTINGS_TABS[nextIndex].id;
}

export function settingsTabIsMounted(active: SettingsTab, tab: SettingsTab): boolean {
  return active === tab;
}

export function createSettingsTabRequestDeduper(
  load: (tab: SettingsTab) => Promise<void>,
): (tab: SettingsTab) => Promise<void> {
  const pending = new Map<SettingsTab, Promise<void>>();

  return (tab) => {
    const inFlight = pending.get(tab);
    if (inFlight) return inFlight;

    const request = load(tab);
    pending.set(tab, request);
    const release = () => {
      if (pending.get(tab) === request) pending.delete(tab);
    };
    void request.then(release, release);
    return request;
  };
}
