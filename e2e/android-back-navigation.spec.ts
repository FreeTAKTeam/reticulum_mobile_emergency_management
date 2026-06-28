import { expect, test } from "@playwright/test";

import {
  resolveAndroidRouteBackAction,
  runBackNavigationHandlers,
  type BackNavigationHandler,
} from "../apps/mobile/src/utils/androidBackNavigation";

test("Android route back uses history before dashboard fallback", () => {
  expect(resolveAndroidRouteBackAction({ canGoBack: true, currentPath: "/events/help" })).toBe("back");
  expect(resolveAndroidRouteBackAction({ canGoBack: false, currentPath: "/events/help" })).toBe("dashboard");
  expect(resolveAndroidRouteBackAction({ canGoBack: false, currentPath: "/dashboard" })).toBe("stay");
  expect(resolveAndroidRouteBackAction({ canGoBack: false, currentPath: "/setup" })).toBe("stay");
});

test("Android back handlers run from newest to oldest until handled", async () => {
  const calls: string[] = [];
  const handlers: BackNavigationHandler[] = [
    () => {
      calls.push("old");
      return true;
    },
    () => {
      calls.push("new");
      return false;
    },
    () => {
      calls.push("newest");
      return true;
    },
  ];

  await expect(runBackNavigationHandlers(handlers)).resolves.toBe(true);
  expect(calls).toEqual(["newest"]);
});

test("Android back handlers report unhandled when no handler consumes it", async () => {
  const calls: string[] = [];
  const handlers: BackNavigationHandler[] = [
    () => {
      calls.push("old");
      return false;
    },
    () => {
      calls.push("new");
      return false;
    },
  ];

  await expect(runBackNavigationHandlers(handlers)).resolves.toBe(false);
  expect(calls).toEqual(["new", "old"]);
});
