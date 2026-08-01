import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { LocaleProvider } from "./lib/i18n";

if (import.meta.env.DEV && new URLSearchParams(window.location.search).has("browser-mock")) {
  const { installBrowserMock } = await import("./test-support/browserMock");
  installBrowserMock();
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <LocaleProvider>
      <App />
    </LocaleProvider>
  </React.StrictMode>,
);
