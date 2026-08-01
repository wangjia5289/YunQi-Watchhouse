import { CategoryRulePreview } from "../../lib/ipc";

export type CategoryRulePreviewState =
  | "NO_MATCHES"
  | "DISABLED"
  | "FULLY_SHADOWED"
  | "PARTIALLY_SHADOWED"
  | "WILL_APPLY";

export function categoryRulePreviewState(
  preview: CategoryRulePreview,
  enabled: boolean,
): CategoryRulePreviewState {
  if (preview.matchedSessionCount === 0) return "NO_MATCHES";
  if (!enabled) return "DISABLED";
  if (preview.effectiveSessionCount === 0 && preview.shadowedSessionCount > 0) {
    return "FULLY_SHADOWED";
  }
  if (preview.shadowedSessionCount > 0) return "PARTIALLY_SHADOWED";
  return "WILL_APPLY";
}
