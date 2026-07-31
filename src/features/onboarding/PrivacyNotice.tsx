interface PrivacyNoticeProps {
  onboarding?: boolean;
  onAccept?: () => void;
  onClose?: () => void;
}

export function PrivacyNotice({
  onboarding = false,
  onAccept,
  onClose,
}: PrivacyNoticeProps) {
  const { t } = useLocale();
  return (
    <div className={onboarding ? "onboarding-overlay" : "privacy-panel"}>
      <section className="privacy-card" role={onboarding ? "dialog" : undefined}
        aria-modal={onboarding || undefined} aria-labelledby="privacy-title">
        <span className="privacy-mark" aria-hidden="true"><i /></span>
        <p className="section-kicker">{t("Private by design")}</p>
        <h1 id="privacy-title">{t(onboarding ? "Welcome to Watchhouse" : "Privacy in Watchhouse")}</h1>
        <p className="privacy-intro">
          {t("Watchhouse creates a private computer activity timeline on this Mac. No account or network connection is required.")}
        </p>
        <div className="privacy-grid">
          <article>
            <strong>{t("What is recorded")}</strong>
            <ul>
              <li>{t("Application name and identifier")}</li>
              <li>{t("Active and idle timestamps")}</li>
              <li>{t("Session duration")}</li>
              <li>{t("Window titles only after global and per-application opt-in")}</li>
            </ul>
          </article>
          <article>
            <strong>{t("What is never recorded")}</strong>
            <ul>
              <li>{t("Keystrokes or typed text")}</li>
              <li>{t("Screenshots or screen recordings")}</li>
              <li>{t("Clipboard contents or passwords")}</li>
            </ul>
          </article>
        </div>
        <div className="privacy-local">
          <strong>{t("Stored locally")}</strong>
          <p>{t("Activity remains in the Watchhouse SQLite database. Diagnostics contain only technical lifecycle and error information.")}</p>
        </div>
        <div className="privacy-actions">
          {onClose && <button className="secondary-action" onClick={onClose}>{t("Close")}</button>}
          {onAccept && <button className="privacy-primary" onClick={onAccept}>{t("Accept and Continue")}</button>}
        </div>
      </section>
    </div>
  );
}
import { useLocale } from "../../lib/i18n";
