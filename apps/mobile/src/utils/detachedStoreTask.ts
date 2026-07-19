export interface DetachedStoreTaskErrorSink {
  setLastError(message: string): void;
  logUi(level: string, message: string): void;
}

export function runDetachedStoreTask(
  sink: DetachedStoreTaskErrorSink,
  scope: string,
  operation: string,
  task: () => Promise<void>,
): void {
  void Promise.resolve().then(task).catch((error: unknown) => {
    const detail = error instanceof Error ? error.message : String(error);
    const message = `[${scope}] ${operation} failed: ${detail}`;
    sink.setLastError(message);
    sink.logUi("Warn", message);
  });
}
