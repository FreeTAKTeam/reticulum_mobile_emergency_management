export const SETUP_WIZARD_STORAGE_KEY = "reticulum.mobile.setupWizard.v1";

export interface SetupWizardState {
  completed: boolean;
  completedAt?: number;
  lastOpenedAt?: number;
}

function readState(): SetupWizardState {
  try {
    const raw = localStorage.getItem(SETUP_WIZARD_STORAGE_KEY);
    if (!raw) {
      return { completed: false };
    }
    const parsed = JSON.parse(raw) as Partial<SetupWizardState>;
    return {
      completed: Boolean(parsed.completed),
      completedAt: typeof parsed.completedAt === "number" ? parsed.completedAt : undefined,
      lastOpenedAt: typeof parsed.lastOpenedAt === "number" ? parsed.lastOpenedAt : undefined,
    };
  } catch {
    return { completed: false };
  }
}

export function loadSetupWizardState(): SetupWizardState {
  return readState();
}

export function hasCompletedSetupWizard(): boolean {
  return readState().completed;
}

export function markSetupWizardOpened(): SetupWizardState {
  const next = {
    ...readState(),
    lastOpenedAt: Date.now(),
  };
  localStorage.setItem(SETUP_WIZARD_STORAGE_KEY, JSON.stringify(next));
  return next;
}

export function markSetupWizardCompleted(): SetupWizardState {
  const now = Date.now();
  const next: SetupWizardState = {
    completed: true,
    completedAt: now,
    lastOpenedAt: now,
  };
  localStorage.setItem(SETUP_WIZARD_STORAGE_KEY, JSON.stringify(next));
  return next;
}
