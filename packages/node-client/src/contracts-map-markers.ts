/** A shared map marker projected from the native r3akt-map-core service. */
export interface MapMarkerRecord {
  objectDestinationHash: string;
  originRch: string;
  markerType: string;
  symbol: string;
  name: string;
  category: string;
  lat: number;
  lon: number;
  notes?: string;
  createdAtMs: number;
  updatedAtMs: number;
}

/** One entry of the backend-owned symbol catalog shared with RCH. */
export interface MapMarkerSymbolRecord {
  id: string;
  set: string;
  mdi?: string;
  description: string;
  tak?: string;
  category: string;
  aliases: string[];
}

/** Creation fields; validation and normalization happen natively. */
export interface MapMarkerCreateRequest {
  idempotencyKey?: string;
  markerType: string;
  symbol: string;
  name?: string;
  category?: string;
  lat: number;
  lon: number;
  notes?: string;
}

export interface MapMarkerCreateResult {
  marker: MapMarkerRecord;
  created: boolean;
}

/**
 * An operator-started plugin map session (plugin API 1.3): the plugin may read only these
 * selected objects until the session stops. The native runtime owns every session rule.
 */
export interface PluginMapSessionRecord {
  sessionId: string;
  pluginId: string;
  objectDestinationHashes: string[];
  startedAtMs: number;
  startedBy: string;
}

export interface PluginMapSessionClient {
  listPluginMapSessions(): Promise<PluginMapSessionRecord[]>;
  startPluginMapSession(pluginId: string, objectDestinationHashes: string[]): Promise<PluginMapSessionRecord>;
  stopPluginMapSession(sessionId: string): Promise<boolean>;
}

export interface MapMarkerClient {
  listMapMarkerSymbols(): Promise<MapMarkerSymbolRecord[]>;
  listMapMarkers(): Promise<MapMarkerRecord[]>;
  createMapMarker(request: MapMarkerCreateRequest): Promise<MapMarkerCreateResult>;
  renameMapMarker(objectDestinationHash: string, name: string): Promise<MapMarkerRecord>;
  moveMapMarker(objectDestinationHash: string, lat: number, lon: number): Promise<MapMarkerRecord>;
  deleteMapMarker(objectDestinationHash: string): Promise<boolean>;
}
