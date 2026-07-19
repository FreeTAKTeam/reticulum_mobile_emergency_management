export interface StartupRuntimeState {
  running: boolean;
  restartRequired: boolean;
}

export interface StartupRuntimeActions {
  start: () => Promise<void>;
  restart: () => Promise<void>;
}

/**
 * Reconcile the foreground service with the application's persisted settings.
 *
 * Android can restore the service before the WebView initializes. Calling the
 * idempotent start operation even when the service is already running ensures
 * stale service configuration is compared with, and replaced by, app state.
 */
export async function reconcileStartupRuntime(
  state: StartupRuntimeState,
  actions: StartupRuntimeActions,
): Promise<void> {
  if (state.running && state.restartRequired) {
    await actions.restart();
    return;
  }

  await actions.start();
}
