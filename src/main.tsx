import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { resolveRootTarget } from "./lib/rootTarget";

const searchParams = new URLSearchParams(window.location.search);
if (import.meta.env.DEV && searchParams.has("browser-mock")) {
  const { installBrowserMock } = await import("./test-support/browserMock");
  installBrowserMock(searchParams.get("window") === "tray-panel" ? "tray-panel" : "main");
}

const tauriAvailable = "__TAURI_INTERNALS__" in window;
const rootTarget = resolveRootTarget(
  tauriAvailable,
  tauriAvailable ? getCurrentWindow().label : undefined,
);
let root: React.ReactNode;
if (rootTarget === "tray-panel") {
  const { TrayPanel } = await import("./features/tray/TrayPanel");
  root = <TrayPanel />;
} else {
  const [{ default: App }, { LocaleProvider }] = await Promise.all([
    import("./App"),
    import("./lib/i18n"),
  ]);
  root = <LocaleProvider><App /></LocaleProvider>;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {root}
  </React.StrictMode>,
);
