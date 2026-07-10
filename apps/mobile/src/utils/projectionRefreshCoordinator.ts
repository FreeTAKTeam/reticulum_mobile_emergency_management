export interface ProjectionRefreshOptions {
  trailing?: boolean;
}

interface ProjectionRefreshState {
  operation: () => Promise<void>;
  promise: Promise<void>;
  trailingRequested: boolean;
}

export class ProjectionRefreshCoordinator {
  private readonly states = new Map<string, ProjectionRefreshState>();

  run(
    key: string,
    operation: () => Promise<void>,
    options: ProjectionRefreshOptions = {},
  ): Promise<void> {
    const normalizedKey = key.trim();
    if (!normalizedKey) {
      return Promise.reject(new Error("Projection refresh key must not be empty."));
    }

    const active = this.states.get(normalizedKey);
    if (active) {
      if (options.trailing) {
        active.operation = operation;
        active.trailingRequested = true;
      }
      return active.promise;
    }

    const state: ProjectionRefreshState = {
      operation,
      promise: Promise.resolve(),
      trailingRequested: false,
    };
    let resolveState: (() => void) | undefined;
    let rejectState: ((reason: unknown) => void) | undefined;
    state.promise = new Promise<void>((resolve, reject) => {
      resolveState = resolve;
      rejectState = reject;
    });
    this.states.set(normalizedKey, state);
    void (async () => {
      let nextOperation = operation;
      do {
        state.trailingRequested = false;
        await nextOperation();
        nextOperation = state.operation;
      } while (state.trailingRequested);
    })().then(
      () => {
        if (this.states.get(normalizedKey) === state) {
          this.states.delete(normalizedKey);
        }
        resolveState?.();
      },
      (error: unknown) => {
        if (this.states.get(normalizedKey) === state) {
          this.states.delete(normalizedKey);
        }
        rejectState?.(error);
      },
    );
    return state.promise;
  }
}

export const projectionRefreshCoordinator = new ProjectionRefreshCoordinator();
