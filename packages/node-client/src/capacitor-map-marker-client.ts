import { toSensorOrigins } from "./sensor-converters";
import type { SensorOriginsSnapshot } from "./contracts-sensors";
import type { MapMarkerClient, MapMarkerCreateRequest, MapMarkerCreateResult, MapMarkerRecord, MapMarkerSymbolRecord, PluginMapSessionClient, PluginMapSessionRecord } from "./contracts-map-markers";
import { CapacitorProjectionClient } from "./capacitor-projection-client";
import { toMapMarkerFromResult, toMapMarkerItems, toMapMarkerRecord, toMapMarkerSymbolRecord, toPluginMapSessionRecord } from "./map-marker-converters";

/** Native map marker calls; the runtime owns every marker rule. */
export abstract class CapacitorMapMarkerClient extends CapacitorProjectionClient implements MapMarkerClient, PluginMapSessionClient {
  async getSensorOrigins(): Promise<SensorOriginsSnapshot> {
    await this.ready();
    return toSensorOrigins(await this.plugin.getSensorOrigins());
  }

  async setSensorVideoSharing(originId: string, sensorId: string, approved: boolean): Promise<void> {
    await this.ready();
    await this.plugin.setSensorVideoSharing({ request: { originId, sensorId, approved } });
  }

  async listMapMarkerSymbols(): Promise<MapMarkerSymbolRecord[]> {
    await this.ready();
    return toMapMarkerItems(await this.plugin.listMapMarkerSymbols()).map(toMapMarkerSymbolRecord);
  }

  async listMapMarkers(): Promise<MapMarkerRecord[]> {
    await this.ready();
    return toMapMarkerItems(await this.plugin.listMapMarkers()).map(toMapMarkerRecord);
  }

  async createMapMarker(request: MapMarkerCreateRequest): Promise<MapMarkerCreateResult> {
    await this.ready();
    const result = await this.plugin.createMapMarker({ request });
    return { marker: toMapMarkerFromResult(result), created: result.created !== false };
  }

  async renameMapMarker(objectDestinationHash: string, name: string): Promise<MapMarkerRecord> {
    await this.ready();
    return toMapMarkerFromResult(
      await this.plugin.renameMapMarker({ request: { objectDestinationHash, name } }),
    );
  }

  async moveMapMarker(objectDestinationHash: string, lat: number, lon: number): Promise<MapMarkerRecord> {
    await this.ready();
    return toMapMarkerFromResult(
      await this.plugin.moveMapMarker({ request: { objectDestinationHash, lat, lon } }),
    );
  }

  async deleteMapMarker(objectDestinationHash: string): Promise<boolean> {
    await this.ready();
    const result = await this.plugin.deleteMapMarker({ objectDestinationHash });
    return result.deleted === true;
  }

  async listPluginMapSessions(): Promise<PluginMapSessionRecord[]> {
    await this.ready();
    return toMapMarkerItems(await this.plugin.listPluginMapSessions()).map(toPluginMapSessionRecord);
  }

  async startPluginMapSession(pluginId: string, objectDestinationHashes: string[]): Promise<PluginMapSessionRecord> {
    await this.ready();
    return toPluginMapSessionRecord(
      await this.plugin.startPluginMapSession({ pluginId, objectDestinationHashes }),
    );
  }

  async stopPluginMapSession(sessionId: string): Promise<boolean> {
    await this.ready();
    const result = await this.plugin.stopPluginMapSession({ sessionId });
    return result.stopped === true;
  }
}
