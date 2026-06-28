export type BackNavigationHandler = () => boolean | Promise<boolean>;

export type AndroidRouteBackAction = "back" | "dashboard" | "stay";

export interface AndroidRouteBackState {
  canGoBack: boolean;
  currentPath: string;
}

const rootPaths = new Set(["/dashboard", "/setup"]);
const backNavigationHandlers: BackNavigationHandler[] = [];

export function registerBackNavigationHandler(handler: BackNavigationHandler): () => void {
  backNavigationHandlers.push(handler);
  return () => {
    const handlerIndex = backNavigationHandlers.lastIndexOf(handler);
    if (handlerIndex >= 0) {
      backNavigationHandlers.splice(handlerIndex, 1);
    }
  };
}

export async function runBackNavigationHandlers(
  handlers: readonly BackNavigationHandler[] = backNavigationHandlers,
): Promise<boolean> {
  for (let index = handlers.length - 1; index >= 0; index -= 1) {
    if (await handlers[index]()) {
      return true;
    }
  }
  return false;
}

export function resolveAndroidRouteBackAction(state: AndroidRouteBackState): AndroidRouteBackAction {
  if (state.canGoBack) {
    return "back";
  }
  if (rootPaths.has(state.currentPath)) {
    return "stay";
  }
  return "dashboard";
}
