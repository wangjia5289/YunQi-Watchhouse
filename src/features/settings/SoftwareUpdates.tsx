import { useEffect, useReducer } from "react";
import {
  checkForUpdates,
  errorMessage,
  installUpdate,
} from "../../lib/ipc";
import { useLocale } from "../../lib/i18n";
import {
  INITIAL_SOFTWARE_UPDATE_STATE,
  reduceSoftwareUpdateState,
} from "./SoftwareUpdatesModel";
import "./SoftwareUpdates.css";

export function SoftwareUpdates() {
  const { t } = useLocale();
  const [{ result, checking, installing, error }, dispatch] = useReducer(
    reduceSoftwareUpdateState,
    INITIAL_SOFTWARE_UPDATE_STATE,
  );

  async function checkNow() {
    dispatch({ type: "check-started" });
    try {
      dispatch({ type: "check-succeeded", result: await checkForUpdates() });
    } catch (reason) {
      dispatch({ type: "check-failed", details: errorMessage(reason) });
    }
  }

  useEffect(() => {
    void checkNow();
  }, []);

  async function install() {
    if (!window.confirm(t("Install the update and restart Watchhouse?"))) return;
    dispatch({ type: "install-started" });
    try {
      await installUpdate();
    } catch (reason) {
      dispatch({ type: "install-failed", details: errorMessage(reason) });
    }
  }

  const status = result && !result.configured
    ? t("Updates are enabled in signed release builds.")
    : result?.available
      ? `${t("Version available:")} ${result.version}`
      : result
        ? t("Watchhouse is up to date.")
        : t("Update status has not been checked.");

  return (
    <section className="settings-card software-updates-card">
      <div className="list-heading software-updates-heading">
        <div>
          <p className="section-kicker">{t("Delivery")}</p>
          <h2>{t("Software updates")}</h2>
        </div>
        {result && <span>{`v${result.currentVersion}`}</span>}
      </div>
      <div className="software-update-row">
        <div>
          <strong>{status}</strong>
          <small>{t("Signed releases check GitHub securely and verify every update before installation.")}</small>
          {result?.available && result.notes && <p>{result.notes}</p>}
          {error && (
            <div className="software-update-error" role="alert">
              <strong>{t(error.summary)}</strong>
              <details>
                <summary>{t("Technical details")}</summary>
                <pre>{error.details}</pre>
              </details>
            </div>
          )}
        </div>
        <div className="software-update-actions">
          <button type="button" disabled={checking || installing} onClick={() => void checkNow()}>
            {t(checking ? "Checking…" : "Check for Updates")}
          </button>
          {result?.available && (
            <button
              type="button"
              className="install"
              disabled={checking || installing}
              onClick={() => void install()}
            >
              {t(installing ? "Installing…" : "Install and Restart")}
            </button>
          )}
        </div>
      </div>
    </section>
  );
}
