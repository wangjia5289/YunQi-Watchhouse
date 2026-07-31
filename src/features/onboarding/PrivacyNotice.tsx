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
  return (
    <div className={onboarding ? "onboarding-overlay" : "privacy-panel"}>
      <section className="privacy-card" role={onboarding ? "dialog" : undefined}
        aria-modal={onboarding || undefined} aria-labelledby="privacy-title">
        <span className="privacy-mark" aria-hidden="true"><i /></span>
        <p className="section-kicker">Private by design</p>
        <h1 id="privacy-title">{onboarding ? "Welcome to Watchhouse" : "Privacy in Watchhouse"}</h1>
        <p className="privacy-intro">
          Watchhouse creates a private computer activity timeline on this Mac.
          No account or network connection is required.
        </p>
        <div className="privacy-grid">
          <article>
            <strong>What is recorded</strong>
            <ul>
              <li>Application name and identifier</li>
              <li>Active and idle timestamps</li>
              <li>Session duration</li>
            </ul>
          </article>
          <article>
            <strong>What is never recorded</strong>
            <ul>
              <li>Keystrokes or typed text</li>
              <li>Screenshots or screen recordings</li>
              <li>Clipboard contents or passwords</li>
            </ul>
          </article>
        </div>
        <div className="privacy-local">
          <strong>Stored locally</strong>
          <p>Activity remains in the Watchhouse SQLite database. Diagnostics contain only technical lifecycle and error information.</p>
        </div>
        <div className="privacy-actions">
          {onClose && <button className="secondary-action" onClick={onClose}>Close</button>}
          {onAccept && <button className="privacy-primary" onClick={onAccept}>Accept and Continue</button>}
        </div>
      </section>
    </div>
  );
}
