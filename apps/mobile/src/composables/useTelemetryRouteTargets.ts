import { computed } from "vue";
import type { RouteLocationNormalizedLoaded } from "vue-router";

import type { TelemetryPosition } from "../types/domain";
import {
  parseSosRouteTarget,
  parseTelemetryRouteTarget,
  safeLower,
  safeTrim,
  sosIdentityKey,
  type TelemetryRouteTarget,
} from "../utils/telemetryMapModel";

export function useTelemetryRouteTargets(
  route: RouteLocationNormalizedLoaded,
  getPositions: () => Record<string, TelemetryPosition>,
) {
  const selectedSosTarget = computed(() => parseSosRouteTarget(route.query));
  const selectedSosTargetKey = computed(() => {
    const target = selectedSosTarget.value;
    return target
      ? `${sosIdentityKey(target.incidentId, target.sourceHex)}:${safeLower(target.messageIdHex)}`
      : "";
  });
  const selectedTelemetryTarget = computed<TelemetryRouteTarget | null>(() => {
    const parsedTarget = parseTelemetryRouteTarget(route.query);
    if (parsedTarget) return parsedTarget;
    const callsign = safeTrim(route.query.callsign);
    const position = getPositions()[safeLower(callsign)];
    return position
      ? { callsign: position.callsign, lat: position.lat, lon: position.lon }
      : null;
  });
  const selectedTelemetryTargetKey = computed(() => {
    const target = selectedTelemetryTarget.value;
    return target ? `${safeLower(target.callsign)}:${target.lat}:${target.lon}` : "";
  });
  return {
    selectedSosTarget,
    selectedSosTargetKey,
    selectedTelemetryTarget,
    selectedTelemetryTargetKey,
  };
}
