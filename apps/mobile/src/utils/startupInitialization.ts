export async function runRecoverableStartupStep(
  label: string,
  operation: () => void | Promise<void>,
  reportFailure: (message: string, error: unknown) => void,
): Promise<boolean> {
  try {
    await operation();
    return true;
  } catch (error: unknown) {
    const detail = error instanceof Error ? error.message : String(error);
    reportFailure(`${label} failed: ${detail}`, error);
    return false;
  }
}

export function isRecoveredChatHydrationError(message: string): boolean {
  const normalized = message.trim().toLowerCase();
  return normalized.startsWith("chat history hydration failed:")
    || normalized.startsWith("chat history hydration retry failed:")
    || normalized.startsWith("[chat] history hydration retry failed:");
}
