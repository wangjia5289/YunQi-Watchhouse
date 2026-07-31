import { UpdateCheck } from "../../lib/ipc";

export interface SoftwareUpdateError {
  summary: "Update check failed." | "Update installation failed.";
  details: string;
}

export interface SoftwareUpdateState {
  result: UpdateCheck | null;
  checking: boolean;
  installing: boolean;
  error: SoftwareUpdateError | null;
}

export type SoftwareUpdateAction =
  | { type: "check-started" }
  | { type: "check-succeeded"; result: UpdateCheck }
  | { type: "check-failed"; details: string }
  | { type: "install-started" }
  | { type: "install-failed"; details: string };

export const INITIAL_SOFTWARE_UPDATE_STATE: SoftwareUpdateState = {
  result: null,
  checking: false,
  installing: false,
  error: null,
};

export function reduceSoftwareUpdateState(
  state: SoftwareUpdateState,
  action: SoftwareUpdateAction,
): SoftwareUpdateState {
  switch (action.type) {
    case "check-started":
      return { ...state, result: null, checking: true, error: null };
    case "check-succeeded":
      return { ...state, result: action.result, checking: false, error: null };
    case "check-failed":
      return {
        ...state,
        result: null,
        checking: false,
        error: { summary: "Update check failed.", details: action.details },
      };
    case "install-started":
      return { ...state, installing: true, error: null };
    case "install-failed":
      return {
        ...state,
        installing: false,
        error: { summary: "Update installation failed.", details: action.details },
      };
  }
}
