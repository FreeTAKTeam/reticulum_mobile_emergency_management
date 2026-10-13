import type { MapMarkerClient, MapMarkerCreateRequest, MapMarkerCreateResult, MapMarkerRecord, MapMarkerSymbolRecord } from "./contracts-map-markers";
import { CapacitorProjectionClient } from "./capacitor-projection-client";
import { toMapMarkerFromResult, toMapMarkerItems, toMapMarkerRecord, toMapMarkerSymbolRecord } from "./map-marker-converters";

/** Native map marker calls; the runtime owns every marker rule. */
export abstract class CapacitorMapMarkerClient extends CapacitorProjectionClient implements MapMarkerClient {
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
}
