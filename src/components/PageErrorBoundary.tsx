import { Component, ErrorInfo, ReactNode } from "react";
import { useLocale } from "../lib/i18n";

interface BoundaryProps {
  children: ReactNode;
  fallback: ReactNode;
  resetKey: string;
}

interface BoundaryState {
  failed: boolean;
}

class RenderBoundary extends Component<BoundaryProps, BoundaryState> {
  state: BoundaryState = { failed: false };

  static getDerivedStateFromError(): BoundaryState {
    return { failed: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("page rendering failed", error, info);
  }

  componentDidUpdate(previous: BoundaryProps) {
    if (this.state.failed && previous.resetKey !== this.props.resetKey) {
      this.setState({ failed: false });
    }
  }

  render() {
    return this.state.failed ? this.props.fallback : this.props.children;
  }
}

export function PageErrorBoundary({
  children,
  resetKey,
}: {
  children: ReactNode;
  resetKey: string;
}) {
  const { t } = useLocale();
  const fallback = (
    <div className="page-load-error" role="alert">
      <span>{t("Unable to load this page.")}</span>
      <button type="button" onClick={() => window.location.reload()}>
        {t("Try again")}
      </button>
    </div>
  );
  return (
    <RenderBoundary fallback={fallback} resetKey={resetKey}>
      {children}
    </RenderBoundary>
  );
}
