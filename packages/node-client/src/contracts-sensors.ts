/** Native R3S projections. Geometry, freshness and synchronization are runtime decisions. */
export interface SensorState {
  sensorId: string;
  sampleAtMs: number;
  azimuthDeg: number | null;
  elevationDeg: number | null;
  rollDeg: number | null;
  horizontalFovDeg: number | null;
  verticalFovDeg: number | null;
  slantRangeM: number | null;
  imageNorthDeg: number | null;
  rangeKind: "unknown" | "measured-target" | "display-limit";
}
export interface OriginPosition {
  lat: number; lon: number; altM: number | null;
  altitudeDatum: "unknown" | "hae" | "msl" | "agl";
  fixAtMs: number; speedMps: number | null; courseDeg: number | null; accuracyM: number | null;
}
export interface SensorGeoJson {
  type: "FeatureCollection";
  features: Array<{type: "Feature"; properties: Record<string, unknown>; geometry: {type: string; coordinates: unknown}}>;
}
export interface SensorProjection {
  state: SensorState;
  manifest: {sensorId: string; label?: string | null; sensorType?: string | null; model?: string | null;
    video?: {protocol: string; endpointRef: string; sensorId: string} | null} | null;
  videoSharingApproved?: boolean;
  display: {stale: boolean; positionStale: boolean; unsynchronized: boolean; rangeUnknown: boolean; sampleAtMs: number; geojson: SensorGeoJson};
}
export interface SensorOriginProjection {
  originId: string; writerId: string; originKind: string; label: string;
  position: OriginPosition | null; sensors: SensorProjection[];
}
export interface SensorOriginsSnapshot {items: SensorOriginProjection[]; observedAtMs: number; ttlMs: number}
export interface SensorGeometryClient {
  getSensorOrigins(): Promise<SensorOriginsSnapshot>;
  setSensorVideoSharing(originId: string, sensorId: string, approved: boolean): Promise<void>;
}
