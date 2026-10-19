/**
 * Setup wizard steps, in the same order as the Settings index. Every step between
 * Welcome and Review renders the form of the matching Settings page.
 */
export type SetupWizardStepId =
  | "welcome"
  | "identity"
  | "network"
  | "routing"
  | "location"
  | "sos"
  | "review";

export interface SetupWizardStep {
  id: SetupWizardStepId;
  label: string;
  /** Optional steps can be passed with their defaults (Routing stays Autonomous). */
  optional?: boolean;
}

export const SETUP_STEPS: SetupWizardStep[] = [
  { id: "welcome", label: "Welcome" },
  { id: "identity", label: "Identity" },
  { id: "network", label: "Network" },
  { id: "routing", label: "Routing", optional: true },
  { id: "location", label: "Location" },
  { id: "sos", label: "SOS" },
  { id: "review", label: "Review" },
];
