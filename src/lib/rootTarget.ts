export type RootTarget = "main" | "tray-panel";

export function resolveRootTarget(
  tauriAvailable: boolean,
  windowLabel?: string,
): RootTarget {
  return tauriAvailable && windowLabel === "tray-panel" ? "tray-panel" : "main";
}
