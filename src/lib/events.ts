export const ACTIVITY_DATA_CHANGED = "watchhouse:activity-data-changed";

export function notifyActivityDataChanged() {
  window.dispatchEvent(new Event(ACTIVITY_DATA_CHANGED));
}
