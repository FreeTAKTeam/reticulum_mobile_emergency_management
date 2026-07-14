import {
  createReticulumNodeClient,
  type ReticulumNodeClient,
} from "@reticulum/node-client";

export type ProjectionClientMode = "auto" | "capacitor";
export type ProjectionClientScope =
  | "checklists"
  | "events"
  | "messages"
  | "messaging"
  | "telemetry";

type ProjectionClientCache = typeof globalThis & {
  __reticulumProjectionClients?: Partial<Record<ProjectionClientScope, ReticulumNodeClient>>;
};

function projectionClientCache(): Partial<Record<ProjectionClientScope, ReticulumNodeClient>> {
  const globalCache = globalThis as ProjectionClientCache;
  globalCache.__reticulumProjectionClients ??= {};
  return globalCache.__reticulumProjectionClients;
}

export function createProjectionClientAccessor(
  scope: ProjectionClientScope,
): (mode: ProjectionClientMode) => ReticulumNodeClient {
  return (mode) => {
    const cache = projectionClientCache();
    cache[scope] ??= createReticulumNodeClient({ mode });
    return cache[scope];
  };
}
